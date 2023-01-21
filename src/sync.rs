use std::{
  collections::HashMap,
  fmt::Debug,
  ops::Deref,
  path::PathBuf,
  sync::{Arc, Mutex},
};

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

/// Storage Context
/// containing operational details
/// such as db root path or uid
pub struct Context {
  db_root_path: PathBuf,
  uid: String,
}

impl Context {
  pub fn new(db_root_path: PathBuf, uid: String) -> Self {
    Self { db_root_path, uid }
  }
}

/// Action trait for Actionable types
/// Implemented types can be used as storage patch objects.
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
    dtime: DateTime<Utc>,
    uid: &str,
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
  // Unique ID
  id: Uuid,
  // Referred ObjectId
  // must be applied on it
  object_id: Uuid,
  // UserID
  uid: String,
  // Applied date and time in Utc
  dtime: DateTime<Utc>,
  // Related commit id
  commit_id: Option<Uuid>,
  // Object actions parent action id //todo! maybe we should remove it?
  parent_action_id: Option<Uuid>,
  // Create(T) or Patch(A)
  action: ActionKind<T, A>,
  // Signature of the initial/patched object as json string
  // Sha1
  object_signature: String,
  // Remote action object signature
  // serialized (ActionObject as json) with none remote_signature
  // Sha1
  remote_signature: Option<String>,
}

impl<
    T: Serialize + for<'de> Deserialize<'de> + Clone + Debug,
    A: ActionExt + Clone,
  > ActionObject<T, A>
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
  // Reset dtime
  // Should apply only when remote update occurs
  fn reset_dtime(&mut self) {
    self.dtime = Utc::now();
  }
}

pub struct Commit {
  id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  comment: String,
  ancestor_id: Uuid,
  serialized_actions: Vec<String>, // ActionObject JSONs in Vec
}

pub struct StorageObject<
  T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
  A: ActionExt<ObjectType = T>,
> {
  // Storage Object unique ID
  id: Uuid,
  // StorageId
  storage_id: String,
  // Remote actions
  remote_actions: Vec<ActionObject<T, A>>,
  // Local actions
  local_actions: Vec<ActionObject<T, A>>,
  // Latest remote object
  remote_object: Option<T>,
  // Latest local object
  local_object: T,
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
    &self.local_object
  }
}

impl<
    T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
    A: ActionExt<ObjectType = T> + Clone,
  > StorageObject<T, A>
{
  // Clear all local changes
  // If object is local (no remote actions and object state)
  // we should not be here. That object should be removed without
  // clearing it.
  pub fn clear_local_changes(&mut self) -> Result<(), String> {
    // Clear all local actions
    self.local_actions.clear();
    // Set local data object to the remote one
    self.local_object = self.remote_object.to_owned().unwrap();
    Ok(())
  }
  // Rebuild local objects
  // Only should use when remote update occurs
  fn rebuild_local_objects(&mut self) -> Result<(), String> {
    // First set local object if we have remote on
    if let Some(remote_object) = &self.remote_object {
      self.local_object = remote_object.to_owned();
    } else {
      return Err("Only remote object can be rebuild".into());
    }
    // Re apply action objects and update their object signature & dtimes
    for action_object in &mut self.local_actions {
      if let ActionKind::Patch(action) = &action_object.action {
        // Create patched data
        let patched_data = action.apply_patch(
          &self.local_object,
          action_object.dtime,
          &action_object.uid,
        )?;
        // Calculate new signature
        let signature = sha1_signature(&patched_data)?;
        // Set new signature
        action_object.object_signature = signature;
        // Reset dtimes
        action_object.reset_dtime();
        // set local object to patched data
        self.local_object = patched_data;
      }
    }
    Ok(())
  }
  // Create action object by providing a Context, Commit and Action object.
  // If Patch returns error, we return it back to the caller
  fn create_action_object(
    &self,
    ctx: &Context,
    commit: &Commit,
    action: ActionKind<T, A>,
  ) -> Result<ActionObject<T, A>, String> {
    let dtime = Utc::now();
    let object_signature = match &action {
      ActionKind::Create(t) => sha1_signature(t)?,
      ActionKind::Patch(t) => sha1_signature(&t.apply_patch(
        &self.local_object,
        dtime,
        &commit.uid,
      )?)?,
    };
    let res = ActionObject {
      id: Uuid::new_v4(),
      object_id: self.id.clone(),
      uid: ctx.uid.to_owned(),
      dtime,
      commit_id: Some(commit.id),
      parent_action_id: self.local_actions.last().map(|i| i.id),
      action,
      object_signature,
      remote_signature: None,
    };
    Ok(res)
  }
  // Add local action object to Storage Object
  fn add_local_action_object(
    &mut self,
    action_object: ActionObject<T, A>,
  ) -> Result<&T, String> {
    // Check if action object is local
    if action_object.is_remote() {
      return Err(
        "Only local action object allowed to be added as local".into(),
      );
    }
    // Check if action object is a patch one
    // ActionKind::Create(T) should be handled at storage level
    if let ActionKind::Patch(action) = &action_object.action {
      // Check parent id
      // This way it works for when no local_actions and parent id must be None
      if action_object.parent_action_id
        != self.local_actions.last().map(|i| i.id)
      {
        return Err("Local patch error. Parent id is wrong".into());
      }
      // Patch T
      let patched_object = action.apply_patch(
        &self.local_object,
        action_object.dtime,
        &action_object.uid,
      )?;
      // Check signature
      if &action_object.object_signature
        != &crate::prelude::sha1_signature(&patched_object)?
      {
        return Err("Local patch signature error!".into());
      }
      // Replace T with the patched one
      self.local_object = patched_object;
      // Insert action object
      self.local_actions.push(action_object);
      // Return patched data as ref
      return Ok(&self.local_object);
    }
    Err("Patch must have Patch action kind!".into())
  }
  // Add remote action object to Storage Object
  // because of pull operation
  fn add_remote_action_object(
    &mut self,
    action_object: ActionObject<T, A>,
  ) -> Result<&T, String> {
    // Check if action object is a remote one
    if !action_object.is_remote() {
      return Err("Only remote action object can be added here".into());
    }
    // Check action object parent id
    if self.remote_actions.last().map(|i| i.id)
      != action_object.parent_action_id
    {
      return Err("Action Object parent id mismatch".into());
    }
    // Check if storage object is a remote one
    if self.remote_object.is_none() {
      return Err(
        "We cannot add remote action object to local storage object".into(),
      );
    }
    // Only ActionKind::Patch(A) can be managed here
    // ActionKind::Create(T) should be managed at storage level
    if let ActionKind::Patch(action) = &action_object.action {
      // Patch T
      let patched_object = action.apply_patch(
        self.remote_object.as_ref().unwrap(),
        action_object.dtime,
        &action_object.uid,
      )?;
      // Check signature
      if &action_object.object_signature
        != &crate::prelude::sha1_signature(&patched_object)?
      {
        return Err("Remote Patch signature error!".into());
      }
      // Check remote signature
      // todo! we should verify
      if action_object.remote_signature.is_none() {
        return Err("Patch remote signature missing!".into());
      }
      // Replace T with the patched one
      self.remote_object = Some(patched_object);
      // Insert action object
      self.remote_actions.push(action_object);
      // Rebuild local action objects
      self.rebuild_local_objects()?;
      // Return current local object
      // Important! We return LOCAL, as its the latest version of our
      // data object.
      return Ok(&self.local_object);
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
  id: String,
  member_ids: Vec<Uuid>,
  members: Vec<StorageObject<T, A>>,
  last_remote_commit_id: Option<Uuid>,
}

impl<
    T: Serialize + for<'de> Deserialize<'de> + Debug + Clone,
    A: ActionExt<ObjectType = T> + Clone,
  > Storage<T, A>
{
  /// Init a storage by providing a repository object
  /// Based on its data it can pull itself, or init itself
  /// as a local repository with initial data
  pub fn init(id: String) -> Result<Self, String> {
    unimplemented!()
  }
  fn get_by_id(&self, id: Uuid) -> Result<ActionObject<T, A>, String> {
    unimplemented!()
  }
  // Create new object storage
  // as local change
  pub fn insert_new_object(
    &self,
    ctx: &Context,
    commit: &mut Commit,
    object: T,
  ) -> Result<&T, String> {
    unimplemented!()
  }
  // Local patch storage object
  pub fn local_patch(
    &self,
    ctx: &Context,
    commit: &mut Commit,
    object_id: Uuid,
    action: A,
  ) -> Result<&T, String> {
    unimplemented!()
  }
  fn apply_local_action_object(
    &self,
    action_object: ActionObject<T, A>,
  ) -> Result<StorageObject<T, A>, String> {
    let object_id = action_object.object_id;
    if let Some(storage_object) =
      &mut self.members.iter().find(|i| i.id == object_id)
    {}
    todo!()
  }
  pub fn filter(
    &self,
    filter_fn: impl Fn(&T) -> bool,
  ) -> Vec<&StorageObject<T, A>> {
    self
      .members
      .iter()
      .filter(|i| filter_fn(&i.local_object))
      .collect()
  }
  pub fn filter_all(
    &self,
    filter_fn: impl Fn(&T) -> bool,
  ) -> Vec<&StorageObject<T, A>> {
    self
      .members
      .iter()
      .filter(|i| filter_fn(&i.local_object))
      .collect()
  }
  // Add remote action object
  // e.g. by watch or manual pull
  pub fn add_remote_action_object_json(
    aob_json_str: &str,
  ) -> Result<(), String> {
    unimplemented!()
  }
  pub fn register(&self, repository: &Repository) -> Result<(), String> {
    repository.add_storage_hook(Box::new(|a: &str| -> Result<(), String> {
      unimplemented!()
    }))?;
    Ok(())
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
  Server { port_number: usize },
  Remote { remote_url: String },
  Local,
}

impl Mode {
  pub fn server(port_number: usize) -> Self {
    Self::Server { port_number }
  }
  pub fn remote(remote_url: String) -> Self {
    Self::Remote { remote_url }
  }
  pub fn local() -> Self {
    Self::Local
  }
}

/// Commit Log
/// contains all the repository related logs
pub struct CommitLog {
  // Contains the remote commit log
  remote: Vec<Commit>,
  // Contains the latest remote commit id by storage_id's
  // HashMap<StorageId, LatestCommitId>
  latest_remote: HashMap<String, Uuid>,
  // Contains the local commit log
  local: Vec<Commit>,
}

impl CommitLog {
  fn init(ctx: &Context) -> Result<Self, String> {
    unimplemented!()
  }
}

struct RepoDetails {
  mode: Mode,
  remote_url: Option<String>,
}

impl RepoDetails {
  fn init(mode: Mode, remote_url: Option<String>) -> Self {
    unimplemented!()
  }
}

pub struct RepoData {
  ctx: Context,
  commit_log: CommitLog,
  repo_details: RepoDetails,
  storage_hooks: Vec<Box<dyn Fn(&str) -> Result<(), String>>>,
}

impl RepoData {
  fn add_storage_hook(
    &mut self,
    hook: Box<dyn Fn(&str) -> Result<(), String>>,
  ) -> Result<(), String> {
    self.storage_hooks.push(hook);
    Ok(())
  }
}

pub struct Repository {
  inner: Arc<Mutex<RepoData>>,
}

impl Repository {
  pub fn init_local(ctx: Context) -> Result<Self, String> {
    let res = Self {
      inner: Arc::new(Mutex::new(RepoData {
        ctx,
        commit_log: todo!(),
        repo_details: todo!(),
        storage_hooks: vec![],
      })),
    };
    Ok(res)
  }
  pub fn init_remote(ctx: Context) -> Result<Self, String> {
    unimplemented!()
  }
  pub fn start_remote_watcher(&self) -> Result<(), String> {
    unimplemented!()
  }
  pub fn start_server(self) -> Result<(), String> {
    unimplemented!()
  }
  fn add_storage_hook(
    &self,
    hook: Box<dyn Fn(&str) -> Result<(), String>>,
  ) -> Result<(), String> {
    self.inner.lock().unwrap().add_storage_hook(hook)
  }
}
