use std::{fmt::Debug, ops::Deref};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::prelude::sha1_signature;

pub trait InitActionObject<A: ActionExt> {
  fn create_init_action_object(
    &self,
    commit: &Commit,
  ) -> Result<ActionObject<Self, A>, String>
  where
    Self: Clone + Serialize + for<'de> Deserialize<'de>,
  {
    let res = ActionObject {
      id: Uuid::new_v4(),
      object_id: Uuid::new_v4(),
      uid: commit.uid.clone(),
      dtime: Utc::now(),
      commit_id: Some(commit.id),
      parent_action_id: None,
      action: ActionKind::Create((*self).clone()),
      object_signature: sha1_signature(&self)?,
      remote_signature: None,
    };
    Ok(res)
  }
}

// Auto implement InitActionObject trait
impl<A: ActionExt, T> InitActionObject<A> for T where
  T: Serialize + Clone + for<'de> Deserialize<'de>
{
}

pub trait ActionExt: Serialize + for<'de> Deserialize<'de> {
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
#[derive(Serialize, Clone)]
enum ActionKind<T: Serialize + for<'de> Deserialize<'de>, A: ActionExt> {
  /// Create a new object with the given
  /// initial T values (No default as default)
  Create(T),
  /// Patch object with action A
  Patch(A),
}

/// ActionObject must be produced by a StorageObject
/// By providing a &Commit and an A: impl ActionExt to it.
#[derive(Serialize, Clone)]
pub struct ActionObject<T: Serialize + for<'de> Deserialize<'de>, A: ActionExt>
{
  id: Uuid,
  object_id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  commit_id: Option<Uuid>,
  parent_action_id: Option<Uuid>,
  action: ActionKind<T, A>,
  object_signature: String,
  remote_signature: Option<String>,
}

impl<T: Serialize + for<'de> Deserialize<'de> + Clone, A: ActionExt + Clone>
  ActionObject<T, A>
{
  // Check if local action_object
  fn is_local(&self) -> bool {
    self.remote_signature.is_none()
  }
  // Check if remote action_object
  fn is_remote(&self) -> bool {
    self.remote_signature.is_some()
  }
  // Check if patch
  fn is_kind_patch(&self) -> bool {
    if let ActionKind::Patch(_) = self.action {
      return true;
    }
    false
  }
  // Check if create
  fn is_kind_create(&self) -> bool {
    if let ActionKind::Create(_) = self.action {
      return true;
    }
    false
  }
  // Check if remote signature correct
  fn has_valid_remote_signature(&self) -> Result<bool, String> {
    if let Some(remote_signature) = &self.remote_signature {
      let self_clone = (*self).clone();
      let without_signature: ActionObject<T, A> = ActionObject {
        remote_signature: None,
        ..self_clone
      };
      let signature = sha1_signature(&without_signature)?;
      return Ok(&signature == remote_signature);
    }
    Ok(false)
  }
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
  actions: Vec<ActionObject<T, A>>,
  object: T,
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
  pub fn data_object(&self) -> &T {
    &self.object
  }
  fn create_action_object(
    &self,
    action: ActionKind<T, A>,
    uid: String,
    commit_id: Uuid,
  ) -> Result<ActionObject<T, A>, String> {
    let object_signature = match &action {
      ActionKind::Create(t) => sha1_signature(t)?,
      ActionKind::Patch(t) => sha1_signature(&t.apply_patch(self)?)?,
    };
    let res = ActionObject {
      id: Uuid::new_v4(),
      object_id: self.id.clone(),
      uid,
      dtime: Utc::now(),
      commit_id: Some(commit_id),
      parent_action_id: self.actions.last().map(|i| i.id),
      action,
      object_signature,
      remote_signature: None,
    };
    Ok(res)
  }
  fn patch(&mut self, action_object: ActionObject<T, A>) -> Result<&T, String> {
    if let ActionKind::Patch(action) = &action_object.action {
      // Patch T
      let patched_object = action.apply_patch(&self)?;
      // Check signature
      if &action_object.object_signature
        != &crate::prelude::sha1_signature(&patched_object)?
      {
        return Err("Patch signature error!".into());
      }
      // Replace T with the patched one
      self.object = patched_object;
      // Insert action object
      self.actions.push(action_object);
      // Return patched data as ref
      return Ok(&self.object);
    }
    Err("Patch must have Patch action kind!".into())
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
      .filter(|i| filter_fn(i.data_object()))
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

// enum ActionA {}

// impl ActionExt for ActionA {
//   type ObjectType = i32;

//   fn apply_patch(
//     &self,
//     object: &Self::ObjectType,
//   ) -> Result<Self::ObjectType, String> {
//     todo!()
//   }
// }

// enum ActionB {}

// impl ActionExt for ActionB {
//   type ObjectType = String;

//   fn apply_patch(
//     &self,
//     object: &Self::ObjectType,
//   ) -> Result<Self::ObjectType, String> {
//     todo!()
//   }
// }

// enum RepositoryAction {
//   A(ActionA),
//   B(ActionB),
// }

// impl ActionExt for RepositoryAction {
//     type ObjectType;

//     fn apply_patch(
//     &self,
//     object: &Self::ObjectType,
//   ) -> Result<Self::ObjectType, String> {
//         todo!()
//     }
// }
