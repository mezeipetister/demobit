use std::fmt::Debug;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub struct Repository {
  members: Vec<Box<dyn StorageMember>>,
}

///
/// Action
///   Storage / Object / Apply

pub trait StorageMember {}

pub trait ActionExt {
  fn path(storage_object: impl StorageMember) -> Result<(), String>;
}

pub trait StorageExt {
  type Object: StorageMember + Serialize + for<'de> Deserialize<'de> + Debug + Clone;
  type Action: ActionExt;
  fn deserialize_action(aob: ActionObject) -> Result<Self::Object, String> {
    serde_json::from_str(&aob.json_str).map_err(|e| e.to_string())
  }
  fn apply_action(&self, aob: ActionObject) -> Result<(), String>;
}

struct Storage<T: Serialize + for<'de> Deserialize<'de> + Debug + Clone> {
  data: Vec<T>,
}

impl<T: Serialize + for<'de> Deserialize<'de> + Debug + Clone> Storage<T> {
  pub fn apply_action(&self, aob: ActionObject) -> Result<(), String> {
    unimplemented!()
  }
}

pub struct ActionObject {
  storage_name: String,
  uid: String,
  dtime: DateTime<Utc>,
  json_str: String,
}

enum Action {}
