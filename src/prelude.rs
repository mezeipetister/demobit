use serde::Serialize;
use sha1::{Digest, Sha1};

pub fn sha1_signature<T: Serialize>(object: &T) -> Result<String, String> {
  // create a Sha1 object
  let mut hasher = Sha1::new();
  // process input message
  hasher.update(serde_json::to_string(object).unwrap());
  // acquire hash digest in the form of GenericArray,
  // which in this case is equivalent to [u8; 20]
  let result = hasher.finalize();
  let res = format!("{:x}", result);
  Ok(res)
}

pub mod path_helper {
  use std::path::{Path, PathBuf};

  use uuid::Uuid;

  use crate::sync::{Context, ContextGuard};

  pub fn storage_object_path(
    ctx: &Context,
    storage_id: &str,
    object_id: Uuid,
  ) -> PathBuf {
    ctx
      .db_root_path
      .join("storage_data")
      .join(storage_id)
      .join(&object_id.as_simple().to_string())
  }

  pub fn storage_details_path(ctx: &Context, storage_id: &str) -> PathBuf {
    ctx.db_root_path.join("storage_details").join(storage_id)
  }

  pub fn commit_log(ctx: &Context) -> PathBuf {
    ctx.db_root_path.join("commit_log")
  }

  pub fn repo_details(ctx: &Context) -> PathBuf {
    ctx.db_root_path.join("repo_details")
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn test_signature() {
    #[derive(Serialize)]
    struct User {
      name: String,
      age: i32,
    }
    let signature = sha1_signature(&User {
      name: "Peti".into(),
      age: 34,
    });
    println!("{:?}", &signature);
    assert_eq!(signature.is_ok(), true);
  }
}
