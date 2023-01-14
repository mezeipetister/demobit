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

/// ActionObject must be produced by a StorageObject
/// By providing a &Commit and an A: impl ActionExt to it.
pub struct ActionObject<T, A: ActionExt> {
  id: Uuid,
  object_id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  commit_id: Option<Uuid>,
  parent_action_id: Option<Uuid>,
  action: ActionKind<T, A>,
  signature: String,
}

pub struct CommitRef<
  T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
  A: ActionExt<ObjectType = T>,
> {
  id: Uuid,
  local_ancestor_id: Uuid,
  actions: Vec<ActionObject<T, A>>,
}

pub struct Commit {
  id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  comment: String,
  ancestor_id: Uuid,
  serialized_actions: Vec<String>, // Action JSONs in Vec
}

pub struct CommitLog {
  remote: Vec<Commit>,
  local: Vec<Commit>,
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
  commit_ref: CommitRef<T, A>,
}

impl<
    T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
    A: ActionExt<ObjectType = T>,
  > Storage<T, A>
{
  /// Init a storage by providing a repository object
  /// Based on its data it can pull itself, or init itself
  /// as a local repository with initial data
  pub fn init(repository: &Repository) -> Result<Self, String> {
    unimplemented!()
  }
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

pub enum Mode {
  Server { port_number: i32 },
  Remote { remote_url: String },
  Local,
}

impl Mode {
  pub fn server(port_number: i32) -> Self {
    Self::Server { port_number }
  }
  pub fn remote(remote_url: String) -> Self {
    Self::Remote { remote_url }
  }
  pub fn local() -> Self {
    Self::Local
  }
}

pub struct Repository {
  mode: Mode,
  local_commits: Vec<Commit>,
  remote_commits: Vec<Commit>,
}

impl Repository {
  pub fn init(mode: Mode) -> Result<Self, String> {
    unimplemented!()
  }
}
