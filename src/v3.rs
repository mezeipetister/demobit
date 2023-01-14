use std::{fmt::Debug, ops::Deref};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub trait ActionExt {
  /// Action can work with this
  /// type
  type ObjectType;
  /// Patch Object and return a patched version of it.
  /// Object is immutable, so we need to update it in a different
  /// step.
  fn apply_patch(
    &self,
    object: &Self::ObjectType,
  ) -> Result<Self::ObjectType, String>;
}

/// Generic acion representation
/// Atomic action kinds with the following states:
/// Create, Patch, Remove, Recover
enum ActionKind<T, A: ActionExt> {
  /// Create a new object with the given
  /// initial T values (No default as default)
  Create(T),
  /// Patch object with action A
  Patch(A),
  /// Logical delete
  Remove,
  /// Recover deleted Object
  Recover,
}

pub struct ActionObject<T, A: ActionExt> {
  id: Uuid,
  object_id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  commit_id: Option<Uuid>,
  parent_action_id: Option<Uuid>,
  action: ActionKind<T, A>,
  signature: Option<()>, // SHA1 signature
}

pub struct Commit<T, A: ActionExt> {
  id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  comment: String,
  actions: Vec<ActionObject<T, A>>,
  parent_commit_id: Uuid,
  signature: Option<()>, // TODO!
}

pub struct CommitLog<T, A: ActionExt> {
  remote: Vec<Commit<T, A>>,
  local: Vec<Commit<T, A>>,
  staging: Vec<ActionObject<T, A>>,
}

pub struct StorageObject<
  T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
  A: ActionExt<ObjectType = T>,
> {
  id: Uuid,
  local: Vec<ActionObject<T, A>>,
  remote: Vec<ActionObject<T, A>>,
  object: T,
  removed: bool,
  created: DateTime<Utc>,
}

/// Implementing deref for StorageObject<T, A>
/// It means we can immutably access underlying object data
impl<
    T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
    A: ActionExt<ObjectType = T>,
  > Deref for StorageObject<T, A>
{
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.object
  }
}

impl<
    T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
    A: ActionExt<ObjectType = T>,
  > StorageObject<T, A>
{
  pub fn is_active(&self) -> bool {
    !self.removed
  }
  pub fn is_removed(&self) -> bool {
    self.removed
  }
  pub fn data_object(&self) -> &T {
    &self.object
  }
  pub fn patch(&self, action: A) -> Result<ActionObject<T, A>, String> {
    let result = action.apply_patch(self)?;
    let res = ActionObject {
      id: Uuid::new_v4(),
      object_id: self.id.clone(),
      uid: todo!(),
      dtime: Utc::now(),
      commit_id: None,
      parent_action_id: todo!(),
      action: ActionKind::Patch(action),
      signature: todo!(),
    };
    Ok(res)
  }
}

/// Generic Storage that can hold Vec<T>
/// and perform patch A operations
pub struct Storage<
  T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
  A: ActionExt<ObjectType = T>,
> {
  members: Vec<StorageObject<T, A>>,
  commit_log: CommitLog<T, A>,
}

impl<
    T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
    A: ActionExt<ObjectType = T>,
  > Storage<T, A>
{
  pub fn create_object(
    &self,
    init_object: T,
  ) -> Result<StorageObject<T, A>, String> {
    unimplemented!()
  }
  pub fn remove_object(&self, object_id: Uuid) -> Result<(), String> {
    unimplemented!()
  }
  pub fn restore_object(&self, object_id: Uuid) -> Result<(), String> {
    unimplemented!()
  }
  pub fn apply_patch(
    &self,
    action_object: ActionObject<T, A>,
  ) -> Result<StorageObject<T, A>, String> {
    unimplemented!()
  }
  pub fn filter(
    &self,
    filter_fn: impl Fn(&T) -> bool,
  ) -> Vec<&StorageObject<T, A>> {
    self
      .members
      .iter()
      .filter(|i| i.is_active() && filter_fn(i.data_object()))
      .collect()
  }
  pub fn filter_all(
    &self,
    filter_fn: impl Fn(&T) -> bool,
  ) -> Vec<&StorageObject<T, A>> {
    self
      .members
      .iter()
      .filter(|i| filter_fn(i.data_object()))
      .collect()
  }
}

/// Implementing deref for Storaget<T, A>
/// It means we can immutably iterate over its objects
impl<
    T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
    A: ActionExt<ObjectType = T>,
  > Deref for Storage<T, A>
{
  type Target = Vec<StorageObject<T, A>>;
  fn deref(&self) -> &Self::Target {
    &self.members
  }
}
