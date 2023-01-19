use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

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
  Ok(bincode::deserialize(&contents).map_err(|e| e.to_string())?)
}

pub fn binary_continuous_read<T: for<'de> Deserialize<'de>>(
  path: PathBuf,
) -> Result<Vec<T>, String> {
  // Try open staging
  let mut res: Vec<T> = Vec::new();
  let f = std::fs::File::open(&path)
    .map_err(|_| format!("No binary file found: {:?}", path))?;
  loop {
    match bincode::deserialize_from(&f) {
      Ok(r) => res.push(r),
      Err(_) => {
        break;
      }
    }
  }
  Ok(res)
}

pub fn binary_update<T: Serialize>(
  path: PathBuf,
  data: T,
) -> Result<(), String> {
  let mut file = OpenOptions::new()
    .read(true)
    .write(true)
    .truncate(true)
    .open(&path)
    .map_err(|_| format!("No bin file found to update: {:?}", &path))?;
  file
    .write_all(&bincode::serialize(&data).map_err(|e| e.to_string())?)
    .map_err(|e| e.to_string())?;
  file.flush().map_err(|e| e.to_string())?;
  Ok(())
}

pub fn binary_continuous_append<T: Serialize>(
  path: PathBuf,
  append_data: T,
) -> Result<(), String> {
  let mut file = std::fs::OpenOptions::new()
    .read(true)
    .write(true)
    .open(&path)
    .map_err(|_| format!("No continuous file found to append: {:?}", &path))?;
  file.seek(SeekFrom::End(0)).unwrap();
  bincode::serialize_into(&file, &append_data).map_err(|e| e.to_string())?;
  file.flush().map_err(|e| e.to_string())?;
  Ok(())
}
