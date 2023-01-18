use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

use crate::{
  context::Context,
  db::Db,
  prelude::{BitError, BitResult},
};

pub trait DataRead: Db {
  fn read(ctx: &Context) -> BitResult<Self::DataType> {
    // Try open staging
    let mut file = OpenOptions::new()
      .read(true)
      .open(ctx.bit_data_path().join(Self::DB_PATH))
      .map_err(|_| BitError::new("No db file found"))?;
    let mut contents = vec![];
    file.read_to_end(&mut contents)?;
    Ok(bincode::deserialize(&contents)?)
  }
}

#[async_trait]
pub trait DataIter: Db {
  type IterOutputType: for<'de> Deserialize<'de>
    + Serialize
    + Send
    + Sync
    + 'static;
  async fn read(ctx: &Context) -> BitResult<Vec<Self::IterOutputType>> {
    let ctx = ctx.to_owned();
    let res = tokio::task::spawn_blocking(move || {
      let mut res: Vec<Self::IterOutputType> = Vec::new();
      let f =
        std::fs::File::open(ctx.bit_data_path().join(Self::DB_PATH)).unwrap();
      loop {
        match bincode::deserialize_from(&f) {
          Ok(r) => res.push(r),
          Err(_) => {
            break;
          }
        }
      }
      res
    })
    .await?;
    Ok(res)
  }
}

#[async_trait]
pub trait DataUpdate: Db {
  async fn update(ctx: &Context, data: Self::DataType) -> BitResult<()> {
    let mut file = OpenOptions::new()
      .read(true)
      .write(true)
      .truncate(true)
      .open(ctx.bit_data_path().join(Self::DB_PATH))
      .await
      .map_err(|_| BitError::new("No staging db file found"))?;
    file.write_all(&bincode::serialize(&data).unwrap()).await?;
    file.flush().await?;
    Ok(())
  }
}

#[async_trait]
pub trait DataAppend: Db {
  type AppendDataType: Serialize + Send + Sync + 'static;
  async fn append(ctx: &Context, data: Self::AppendDataType) -> BitResult<()> {
    let ctx = ctx.to_owned();
    let res: BitResult<()> = tokio::task::spawn_blocking(move || {
      let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(ctx.bit_data_path().join(Self::DB_PATH))
        .map_err(|_| BitError::new("No REMOTE db file found"))?;
      file.seek(SeekFrom::End(0)).unwrap();
      bincode::serialize_into(&file, &data).unwrap();
      file.flush().unwrap();
      Ok(())
    })
    .await?;
    res?;
    Ok(())
  }
}
