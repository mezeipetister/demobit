use std::{
  fmt::Debug,
  ops::Deref,
  path::PathBuf,
  sync::{Arc, Mutex, MutexGuard},
};

use chrono::{DateTime, Utc};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tonic::{transport::Server, Request};
use uuid::Uuid;

use crate::{
  fs::{
    binary_continuous_append, binary_continuous_read,
    binary_continuous_read_after_filter, binary_init, binary_init_empty,
    binary_read, binary_update,
  },
  prelude::{path_helper, sha1_signature},
  server::sync_api::{
    api_client::ApiClient, api_server::ApiServer, CommitObj, PullRequest,
  },
};

/// Action trait for Actionable types
/// Implemented types can be used as storage patch objects.
pub trait ActionExt: Clone + Send {
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
  /// Human readable display msg
  /// This can be used in UI to display
  /// Patch actions
  fn display(&self) -> String;
}

pub trait ObjectExt: Debug + Clone + Send {}

/// Generic acion representation
/// Atomic action kinds with the following states:
/// Create, Patch, Remove, Recover
#[derive(Serialize, Deserialize, Clone, Debug)]
enum ActionKind<T, A>
where
  T: ObjectExt,
  A: ActionExt,
{
  /// Create a new object with the given
  /// initial T values (No default as default)
  Create(T),
  /// Patch object with action A
  Patch(A),
}

/// ActionObject must be produced by a StorageObject
/// By providing a &Commit and an A: impl ActionExt to it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionObject<T, A>
where
  T: ObjectExt,
  A: ActionExt,
{
  // Unique ID
  id: Uuid,
  // Referred Storage ID
  // Object must be located under it
  storage_id: String,
  // Referred ObjectId
  // must be applied on it
  object_id: Uuid,
  // UserID
  uid: String,
  // Applied date and time in Utc
  dtime: DateTime<Utc>,
  // Related commit id
  commit_id: Option<Uuid>,
  // Object actions parent action id
  // We can use this attribute to check action chain per storage object
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

impl<T, A> ActionObject<T, A>
where
  T: ObjectExt + Serialize,
  A: ActionExt + Serialize,
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

/// Universal Action Object
/// Deserializing Action Object without any action kind type
#[derive(Serialize, Deserialize)]
pub struct UniversalActionObject {
  // Unique ID
  id: Uuid,
  // Referred Storage ID
  // Object must be located under it
  storage_id: String,
  // Referred ObjectId
  // must be applied on it
  object_id: Uuid,
  // UserID
  uid: String,
  // Applied date and time in Utc
  dtime: DateTime<Utc>,
  // Related commit id
  commit_id: Option<Uuid>,
  // Object actions parent action id
  // We can use this attribute to check action chain per storage object
  parent_action_id: Option<Uuid>,
  // Action as AnyValue
  action: Value,
  // Signature of the initial/patched object as json string
  // Sha1
  object_signature: String,
  // Remote action object signature
  // serialized (ActionObject as json) with none remote_signature
  // Sha1
  remote_signature: Option<String>,
}

impl UniversalActionObject {
  fn id(&self) -> Uuid {
    self.id
  }
  fn object_id(&self) -> Uuid {
    self.object_id
  }
  fn parent_action_id(&self) -> Option<Uuid> {
    self.parent_action_id
  }
  fn object_signature(&self) -> &str {
    &self.object_signature
  }
  fn remote_signature(&self) -> Option<&str> {
    self.remote_signature.as_deref()
  }
  fn is_remote(&self) -> bool {
    self.remote_signature.is_some()
  }
  fn is_local(&self) -> bool {
    !self.is_remote()
  }
  fn remote_sign(&mut self) -> Result<(), String> {
    if self.is_remote() {
      return Err("Already signed action object".to_string());
    }
    let signature = sha1_signature(&self)?;
    self.remote_signature = Some(signature);
    Ok(())
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Commit {
  id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  comment: String,
  ancestor_id: Uuid,
  serialized_actions: Vec<String>, // ActionObject JSONs in Vec
  remote_signature: Option<String>, // Remote signature
}

impl Commit {
  fn new(uid: String, comment: String) -> Self {
    Self {
      id: Uuid::new_v4(),
      uid,
      dtime: Utc::now(),
      comment,
      ancestor_id: Uuid::default(),
      serialized_actions: vec![],
      remote_signature: None,
    }
  }
  fn add_action_object(&mut self, aob: impl Serialize) {
    self
      .serialized_actions
      .push(serde_json::to_string(&aob).unwrap());
  }
  fn set_dtime(&mut self) {
    self.dtime = Utc::now()
  }
  fn set_ancestor_id(&mut self, ancestor_id: Uuid) {
    self.ancestor_id = ancestor_id;
  }
  fn is_remote(&self) -> bool {
    self.remote_signature.is_some()
  }
  fn is_local(&self) -> bool {
    !self.is_remote()
  }
  fn add_remote_signature(&mut self) -> Result<(), String> {
    if self.is_remote() {
      return Err("Commit already has remote signature!".into());
    }
    let signature = sha1_signature(&self)?;
    self.remote_signature = Some(signature);
    Ok(())
  }
  fn has_valid_remote_signature(&self) -> Result<bool, String> {
    let mut copied = self.clone();
    let sig1 = copied.remote_signature.take();
    let sig2 = sha1_signature(&self)?;
    if let Some(sig1) = sig1 {
      if sig1 == sig2 {
        return Ok(true);
      }
    }
    Ok(false)
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageObject<T, A>
where
  T: ObjectExt,
  A: ActionExt<ObjectType = T>,
{
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
impl<T, A> Deref for StorageObject<T, A>
where
  T: ObjectExt,
  A: ActionExt<ObjectType = T>,
{
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.local_object
  }
}

impl<T, A> StorageObject<T, A>
where
  T: ObjectExt + Serialize + for<'de> Deserialize<'de>,
  A: ActionExt<ObjectType = T> + Serialize + for<'de> Deserialize<'de> + Debug,
{
  /// Create ActionObject from Action
  /// and add it to the given Commit
  pub fn patch(
    &self,
    action: A,
    commit: &mut CommitContextGuard,
  ) -> Result<(), String> {
    let aob = self.create_action_object(
      &commit.ctx,
      &commit.temp_commit,
      ActionKind::Patch(action),
    )?;
    commit.add_action_object(aob);
    Ok(())
  }
  // Create new Storage Object by providing a ActionKind::Create
  // Action Object
  fn new_from_aob(aob: ActionObject<T, A>) -> Result<Self, String> {
    if let ActionKind::Create(data) = aob.action.clone() {
      let res = match aob.is_local() {
        true => Self {
          id: aob.object_id,
          storage_id: aob.storage_id.clone(),
          remote_actions: vec![],
          local_actions: vec![aob],
          remote_object: None,
          local_object: data,
        },
        false => Self {
          id: aob.object_id,
          storage_id: aob.storage_id.clone(),
          remote_actions: vec![aob],
          local_actions: vec![],
          remote_object: Some(data.clone()),
          local_object: data,
        },
      };
      return Ok(res);
    }
    Err("Action Ojbect must be create kind".into())
  }
  // Check wether StorageObject is only local
  // True if no remote object
  fn is_local_object(&self) -> bool {
    self.remote_object.is_none()
  }
  // Check wether StorageObject is remote
  // True if Some remote object
  fn is_remote_object(&self) -> bool {
    !self.is_local_object()
  }
  // Clear all local changes
  // If object is local (no remote actions and object state)
  // we should not be here. That object should be removed without
  // clearing it.
  pub fn clear_local_changes(&mut self) -> Result<(), String> {
    // Check if remote
    assert!(
      self.is_remote_object(),
      "Only remote StorageObject can be cleared locally"
    );
    // Clear all local actions
    self.local_actions.clear();
    // Set local data object to the remote one
    self.local_object = self.remote_object.to_owned().unwrap();
    Ok(())
  }
  // Rebuild local objects
  // Only should use when remote update occurs
  fn rebuild_local_objects(&mut self) -> Result<(), String> {
    // First set remote object as local one
    if let Some(remote_object) = &self.remote_object {
      self.local_object = remote_object.to_owned();
    } else {
      return Err("Only remote object can be rebuild".to_string());
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
      storage_id: self.storage_id.clone(),
      object_id: self.id.clone(),
      uid: ctx.uid.to_owned(),
      dtime,
      commit_id: Some(commit.id),
      parent_action_id: self.local_actions.last().map(|i| i.id),
      action,
      object_signature,
      remote_signature: None, // todo! This is really None always here? Can remote apply here?
    };
    Ok(res)
  }
  // Add action object
  fn add_action_object(
    &mut self,
    action_object: ActionObject<T, A>,
  ) -> Result<Self, String> {
    if action_object.is_local() {
      return self.add_local_action_object(action_object);
    } else {
      return self.add_remote_action_object(action_object);
    }
  }
  // Add local action object to Storage Object
  fn add_local_action_object(
    &mut self,
    action_object: ActionObject<T, A>,
  ) -> Result<Self, String> {
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
      // Save to fs
      // self.save_to_fs(ctx)?;
      // Return patched StorageObject as ref
      return Ok(self.to_owned());
    }
    Err("Patch must have Patch action kind!".into())
  }
  // Add remote action object to Storage Object
  // because of pull operation
  fn add_remote_action_object(
    &mut self,
    action_object: ActionObject<T, A>,
  ) -> Result<Self, String> {
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
      // Save to FS
      // self.save_to_fs(ctx)?;
      // Return current local object
      // Important! We return LOCAL, as its the latest version of our
      // data object.
      return Ok(self.to_owned());
    }
    Err("Patch must have Patch action kind!".into())
  }
  // Init storage object from FS
  fn read_from_fs(
    ctx: &Context,
    storage_id: &str,
    object_id: Uuid,
  ) -> Result<Self, String> {
    binary_read(path_helper::storage_object_path(ctx, storage_id, object_id))
  }
  // Update storage object file
  fn save_to_fs(&self, ctx: &Context) -> Result<(), String> {
    let object_path =
      path_helper::storage_object_path(ctx, &self.storage_id, self.id);
    binary_update(object_path, &self)
  }
}

/// Generic Storage that can hold Vec<T>
/// and perform patch A operations
#[derive(Clone, Debug)]
pub struct Storage<T, A>
where
  T: ObjectExt,
  A: ActionExt<ObjectType = T>,
{
  inner: Arc<Mutex<StorageInner<T, A>>>,
}

impl<T, A> Deref for Storage<T, A>
where
  T: ObjectExt,
  A: ActionExt<ObjectType = T>,
{
  type Target = Mutex<StorageInner<T, A>>;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StorageInner<T, A>
where
  T: ObjectExt,
  A: ActionExt<ObjectType = T>,
{
  id: String,
  member_ids: Vec<Uuid>,
  members: Vec<StorageObject<T, A>>,
}

impl<T, A> Storage<T, A>
where
  T: ObjectExt + Serialize + for<'de> Deserialize<'de> + 'static,
  A: ActionExt<ObjectType = T>
    + Serialize
    + for<'de> Deserialize<'de>
    + 'static
    + Debug,
{
  /// Init a storage by providing a repository object
  /// Based on its data it can pull itself, or init itself
  /// as a local repository with initial data
  pub fn load_or_init(
    repo: &Repository,
    storage_id: String,
  ) -> Result<Self, String> {
    let ctx = repo.ctx();
    let storage_details_path =
      path_helper::storage_details_path(&ctx, &storage_id);
    let inner: StorageInner<T, A> = match storage_details_path.exists() {
      true => binary_read(storage_details_path)?,
      false => binary_init(
        storage_details_path,
        StorageInner {
          id: storage_id,
          member_ids: Vec::default(),
          members: Vec::default(),
        },
      )?,
    };
    Ok(Self {
      inner: Arc::new(Mutex::new(inner)),
    })
  }

  fn storage_id(&self) -> String {
    self.inner.lock().unwrap().id.to_owned()
  }

  // Get a single storage object by object id
  pub fn get_object_by_id(
    &self,
    ctx: &Context,
    object_id: Uuid,
  ) -> Result<StorageObject<T, A>, String> {
    // Check whether id is member
    if self
      .inner
      .lock()
      .unwrap()
      .member_ids
      .iter()
      .find(|i| **i == object_id)
      .is_none()
    {
      return Err(format!(
        "Storage does not have a member with id {}",
        object_id
      ));
    }
    // read binary
    StorageObject::read_from_fs(ctx, &self.inner.lock().unwrap().id, object_id)
  }

  // Get All
  pub fn get_all(
    &self,
    ctx: &Context,
  ) -> Result<Vec<StorageObject<T, A>>, String> {
    let ids = self.inner.lock().unwrap().member_ids.clone();
    let mut res = Vec::new();
    for id in ids {
      res.push(self.get_object_by_id(ctx, id)?);
    }
    Ok(res)
  }

  // Get by filter
  pub fn get_first_by_filter(
    &self,
    ctx: &Context,
    filter: impl Fn(&T) -> bool,
  ) -> Result<StorageObject<T, A>, String> {
    let ids = self.inner.lock().unwrap().member_ids.clone();
    let mut res = Vec::new();
    for id in ids {
      let so = self.get_object_by_id(ctx, id)?;
      if filter(&so) {
        res.push(so);
      }
    }
    res.first().cloned().ok_or("Object not found".into())
  }

  // Get by filter
  pub fn get_by_filter(
    &self,
    ctx: &Context,
    filter: impl Fn(&T) -> bool,
  ) -> Result<Vec<StorageObject<T, A>>, String> {
    let ids = self.inner.lock().unwrap().member_ids.clone();
    let mut res = Vec::new();
    for id in ids {
      let so = self.get_object_by_id(ctx, id)?;
      if filter(&so) {
        res.push(so);
      }
    }
    Ok(res)
  }

  /// Get by filter
  /// Apply a given patch to result vec items
  pub fn patch_by_filter(
    &self,
    ctx: &mut CommitContextGuard,
    filter: impl Fn(&T) -> bool,
    patch: A,
  ) -> Result<(), String> {
    let res = self.get_by_filter(ctx, filter)?;
    for r in res {
      r.patch(patch.clone(), ctx)?;
    }
    Ok(())
  }

  /// Create a Create action object which will create
  /// a new Storage Object
  /// and adds it to a given Commit
  pub fn create_object(&self, data: T, commit: &mut CommitContextGuard) {
    let object_signature = sha1_signature(&data).unwrap();
    let aob: ActionObject<T, A> = ActionObject {
      id: Uuid::new_v4(),
      storage_id: self.storage_id(),
      object_id: Uuid::new_v4(),
      uid: commit.ctx.uid.to_string(),
      dtime: Utc::now(),
      commit_id: Some(commit.temp_commit.id),
      parent_action_id: None,
      action: ActionKind::Create(data),
      object_signature,
      remote_signature: None,
    };
    commit.add_action_object(aob);
  }

  // Add action object to storage object
  pub fn add_action_object(
    &self,
    ctx: &Context,
    action_object: ActionObject<T, A>,
  ) -> Result<StorageObject<T, A>, String> {
    let object_id = action_object.object_id;
    // Create a new one
    let data = match action_object.is_kind_create() {
      true => {
        // Create new storage object
        let new_storage_object = StorageObject::new_from_aob(action_object)?;
        // Check storage id
        if &self.storage_id() != &new_storage_object.storage_id {
          panic!("Wrong storage id during creating storage object");
        }
        // Get data
        let data = new_storage_object.clone();
        // Get Object path
        let path = path_helper::storage_object_path(
          ctx,
          &new_storage_object.storage_id,
          new_storage_object.id,
        );
        // Init in FS and save its content as binary
        binary_init(path, new_storage_object)?;
        // Add new object ID as storage member ID
        self.inner.lock().unwrap().member_ids.push(object_id);
        // Return data
        data
      }
      // Try to patch existing one
      false => self
        .get_object_by_id(ctx, object_id)?
        .add_action_object(action_object)?
        .clone(),
    };
    Ok(data)
  }

  fn update_fs(&self, ctx: &Context) -> Result<(), String> {
    binary_update(
      path_helper::storage_details_path(ctx, &self.storage_id()),
      self.inner.lock().unwrap().deref(),
    )
  }

  /// Register a callback to a given repository
  /// Repository will use this callback to update storage
  pub fn register(self, repo: &Repository) -> Result<Self, String> {
    let _self = self.clone();
    let ctx = repo.ctx().deref().to_owned();
    repo.add_storage_hook(Box::new(
      move |aobstr: &str,
            callback_mode: CallbackMode|
            -> Option<Result<(), String>> {
        // Try to deserialize action object
        if let Ok(aob) = serde_json::from_str::<ActionObject<T, A>>(aobstr) {
          // Check if storage target is ok
          if &aob.storage_id != &self.storage_id() {
            return None;
          }
          match self.add_action_object(&ctx, aob) {
            Ok(aob) => {
              // Save updated storage object if needed
              match callback_mode {
                CallbackMode::Apply => {
                  aob
                    .save_to_fs(&ctx)
                    .expect("Error writing StorageObject update to fs");
                  let res = self.update_fs(&ctx);
                  return Some(res);
                }
                CallbackMode::Check => Some(Ok::<(), String>(())),
              }
            }
            Err(e) => return Some(Err(e)),
          };
        }
        None
      },
    ))?;
    Ok(_self)
  }
}

// Repository Mode
// Local, Remote or Server
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Mode {
  Server { server_addr: String },
  Remote { remote_url: String },
  Local,
}

impl Mode {
  pub fn server(server_addr: String) -> Self {
    Self::Server { server_addr }
  }
  pub fn remote(remote_url: String) -> Self {
    Self::Remote { remote_url }
  }
  pub fn local() -> Self {
    Self::Local
  }
}

/// Storage Context
/// containing operational details
/// such as db root path or uid
pub struct ContextGuard<'a> {
  mutex_guard: MutexGuard<'a, Context>,
}

impl<'a> Deref for ContextGuard<'a> {
  type Target = Context;

  fn deref(&self) -> &Self::Target {
    self.mutex_guard.deref()
  }
}

impl<'a> ContextGuard<'a> {
  fn new(mutex_guard: MutexGuard<'static, Context>) -> Self {
    Self { mutex_guard }
  }
}

#[derive(Clone)]
pub struct Context {
  pub db_root_path: PathBuf,
  pub uid: String,
}

impl Context {
  pub fn init(db_root_path: PathBuf, uid: String) -> Self {
    Self { db_root_path, uid }
  }
}

pub struct CommitContextGuard<'a> {
  ctx: MutexGuard<'a, Context>,
  commit_log: MutexGuard<'a, CommitLog>,
  repo_details: MutexGuard<'a, RepoDetails>,
  storage_hooks: MutexGuard<
    'a,
    Vec<Box<dyn Fn(&str, CallbackMode) -> Option<Result<(), String>> + Send>>,
  >,
  temp_commit: Commit,
}

impl<'a> Deref for CommitContextGuard<'a> {
  type Target = MutexGuard<'a, Context>;

  fn deref(&self) -> &Self::Target {
    &self.ctx
  }
}

impl<'a> CommitContextGuard<'a> {
  fn new(repo: &'a Repository, commit_comment: &str) -> Self {
    let uid = repo.ctx.lock().unwrap().uid.to_string();
    Self {
      ctx: repo.ctx.lock().unwrap(),
      commit_log: repo.commit_log.lock().unwrap(),
      repo_details: repo.repo_details.lock().unwrap(),
      storage_hooks: repo.storage_hooks.lock().unwrap(),
      temp_commit: Commit::new(uid, commit_comment.to_string()),
    }
  }
  // Use only for server side merge request
  fn new_merge(repo: &'a Repository, temp_commit: Commit) -> Self {
    Self {
      ctx: repo.ctx.lock().unwrap(),
      commit_log: repo.commit_log.lock().unwrap(),
      repo_details: repo.repo_details.lock().unwrap(),
      storage_hooks: repo.storage_hooks.lock().unwrap(),
      temp_commit,
    }
  }
  pub fn add_action_object<
    T: ObjectExt + Serialize,
    A: ActionExt + Serialize,
  >(
    &mut self,
    aob: ActionObject<T, A>,
  ) {
    let _ = self.temp_commit.add_action_object(aob);
  }
}

impl<'a> Drop for CommitContextGuard<'a> {
  fn drop(&mut self) {
    match self.temp_commit.remote_signature.is_some() {
      // Store remote commit
      true => {
        CommitLog::add_remote_commit(&self.ctx, self.temp_commit.clone())
          .expect("Error adding remote commit to commit file");
      }
      // Store local commit
      false => {
        CommitLog::add_local_commit(&self.ctx, self.temp_commit.clone())
          .expect("Error adding local commit to commit file");
      }
    }
    println!("Start apply");
    for aob_str in &self.temp_commit.serialized_actions {
      println!("Iter. Hook count {}", self.storage_hooks.len());
      for hook in self.storage_hooks.deref() {
        let res = hook(aob_str, CallbackMode::Apply);
        println!("Fs result {:?}", &res);
        if res.is_some() {
          break;
        }
      }
    }
    println!("Drop finished");
  }
}

#[derive(Default, Serialize, Deserialize, Debug)]
struct CommitIndex {
  latest_local_commit_id: Option<Uuid>,
  latest_remote_commit_id: Option<Uuid>,
}

impl CommitIndex {
  fn init(ctx: &Context) {
    binary_init(path_helper::commit_index(ctx), Self::default());
  }
  fn load(ctx: &Context) -> Self {
    binary_read(path_helper::commit_index(&ctx))
      .expect("Error reading commit index")
  }
  fn save_fs(&self, ctx: &Context) -> Result<(), String> {
    binary_update(path_helper::commit_index(ctx), &self)
  }
  fn latest_local_commit_id(ctx: &Context) -> Option<Uuid> {
    let s = Self::load(ctx);
    s.latest_local_commit_id
  }
  fn latest_remote_commit_id(ctx: &Context) -> Option<Uuid> {
    let s = Self::load(ctx);
    s.latest_local_commit_id
  }
  fn set_latest_local_id(
    ctx: &Context,
    latest_local: Option<Uuid>,
  ) -> Result<(), String> {
    let mut s = Self::load(ctx);
    s.latest_local_commit_id = latest_local;
    s.save_fs(ctx)
  }
  fn set_latest_remote_id(
    ctx: &Context,
    latest_remote: Option<Uuid>,
  ) -> Result<(), String> {
    let mut s = Self::load(ctx);
    s.latest_remote_commit_id = latest_remote;
    s.save_fs(ctx)
  }
}

/// Commit Log
/// contains all the repository related logs
#[derive(Default, Serialize, Deserialize, Debug)]
pub struct CommitLog;

impl CommitLog {
  fn init(ctx: &Context) -> Result<(), String> {
    // Init latest log
    // binary_init::<HashMap<String, Uuid>>(
    //   path_helper::commit_latest(ctx),
    //   HashMap::default(),
    // )?;
    // Init local log
    binary_init_empty(path_helper::commit_local_log(ctx))?;
    // Init remote log
    binary_init_empty(path_helper::commit_remote_log(ctx))?;
    // Init commit index
    CommitIndex::init(ctx);
    Ok(())
  }

  fn load_locals(ctx: &Context) -> Result<Vec<Commit>, String> {
    let locals = binary_continuous_read(path_helper::commit_local_log(ctx))?;
    Ok(locals)
  }
  fn load_remotes(ctx: &Context) -> Result<Vec<Commit>, String> {
    let remotes = binary_continuous_read(path_helper::commit_remote_log(ctx))?;
    Ok(remotes)
  }
  fn load_remotes_after(
    ctx: &Context,
    after_id: Uuid,
  ) -> Result<Vec<Commit>, String> {
    let remotes = binary_continuous_read_after_filter(
      path_helper::commit_remote_log(ctx),
      |i: &Commit| i.id == after_id,
    )?;
    Ok(remotes)
  }
  fn add_local_commit(
    ctx: &Context,
    mut local_commit: Commit,
  ) -> Result<(), String> {
    // Set ancestor ID
    if let Some(last_local_commit_id) = CommitIndex::latest_local_commit_id(ctx)
    {
      local_commit.set_ancestor_id(last_local_commit_id);
    }
    // Set commit index
    CommitIndex::set_latest_local_id(ctx, Some(local_commit.id))?;
    // Save local commit
    binary_continuous_append(path_helper::commit_local_log(ctx), local_commit)
  }
  fn add_remote_commit(
    ctx: &Context,
    remote_commit: Commit,
  ) -> Result<(), String> {
    let mut commit_index = CommitIndex::load(ctx);
    // check ancestor ID
    if let Some(last_remote_commit_id) = commit_index.latest_remote_commit_id {
      if remote_commit.ancestor_id != last_remote_commit_id {
        return Err("Remote commit ancestor ID error! Please pull".into());
      }
    }
    // Set commit index
    CommitIndex::set_latest_remote_id(ctx, Some(remote_commit.id))?;
    // Save remote commit
    binary_continuous_append(path_helper::commit_remote_log(ctx), remote_commit)
  }
}

#[derive(Serialize, Deserialize, Debug)]
struct RepoDetails {
  mode: Mode,
}

impl RepoDetails {
  fn init(ctx: &Context, mode: Mode) -> Result<(), String> {
    binary_init(path_helper::repo_details(ctx), RepoDetails { mode })?;
    Ok(())
  }
  fn load(ctx: &Context) -> Result<Self, String> {
    binary_read(path_helper::repo_details(ctx))
  }
}

enum CallbackMode {
  Check,
  Apply,
}

pub struct Repository {
  ctx: Arc<Mutex<Context>>,
  commit_log: Arc<Mutex<CommitLog>>,
  repo_details: Arc<Mutex<RepoDetails>>,
  storage_hooks: Arc<
    Mutex<
      Vec<Box<dyn Fn(&str, CallbackMode) -> Option<Result<(), String>> + Send>>,
    >,
  >,
}

impl Repository {
  /// Load repository
  pub fn load(ctx: Context) -> Result<Self, String> {
    // Load commit log
    let commit_log = CommitLog;
    // Load repo details
    let repo_details = RepoDetails::load(&ctx)?;
    // Create res
    let res = Self {
      ctx: Arc::new(Mutex::new(ctx)),
      commit_log: Arc::new(Mutex::new(commit_log)),
      repo_details: Arc::new(Mutex::new(repo_details)),
      storage_hooks: Arc::new(Mutex::new(vec![])),
    };
    Ok(res)
  }
  /// Init repository
  pub fn init(ctx: Context, mode: Mode) -> Result<Self, String> {
    // Check if repository inited
    if Self::load(ctx.clone()).is_ok() {
      return Err("Existing repository. Cannot init a new one".into());
    }
    // Init commit log
    CommitLog::init(&ctx)?;
    // Load commit log
    let commit_log = CommitLog;
    // Init repo details
    RepoDetails::init(&ctx, mode)?;
    // Load repo details
    let repo_details = RepoDetails::load(&ctx)?;
    // Create res
    let res = Self {
      ctx: Arc::new(Mutex::new(ctx)),
      commit_log: Arc::new(Mutex::new(commit_log)),
      repo_details: Arc::new(Mutex::new(repo_details)),
      storage_hooks: Arc::new(Mutex::new(vec![])),
    };
    Ok(res)
  }
  // Clone remote repository to local
  fn clone(remote_url: &str) -> Result<Self, String> {
    // TODO! Fix path and UID
    let ctx = Context::init(PathBuf::from("./data"), "mezeipetister".into());
    // Check if repository inited
    if Self::load(ctx.clone()).is_ok() {
      return Err("Existing repository. Cannot clone again".into());
    }
    unimplemented!()
  }
  /// Pull remote repository
  pub fn proceed_pull(&self) -> Result<(), String> {
    let remote_addr = match &self.repo_details.lock().unwrap().mode {
      Mode::Remote { remote_url } => remote_url.to_string(),
      _ => {
        panic!("Cannot proceed pull operation, as the repository is not in remote mode")
      }
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .worker_threads(1)
      .thread_name("sync_server")
      .build()
      .unwrap();

    runtime.block_on(async {
      let mut remote_client = ApiClient::connect(remote_addr)
        .await
        .expect("Could not connect to UPL service");

      let mut res = remote_client
        .pull(PullRequest {
          after_commit_id: "".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

      let mut commits = vec![];

      while let Some(commit) = res.message().await.unwrap() {
        commits.push(commit);
      }
    });

    Ok(())
  }
  /// Push repository local commits to remote
  pub fn proceed_push(&self) -> Result<(), String> {
    let remote_addr = match &self.repo_details.lock().unwrap().mode {
      Mode::Remote { remote_url } => remote_url.to_string(),
      _ => {
        panic!("Cannot proceed push operation, as the repository is not in remote mode")
      }
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .worker_threads(1)
      .thread_name("sync_server")
      .build()
      .unwrap();

    runtime.block_on(async {
      let mut remote_client = ApiClient::connect(remote_addr)
        .await
        .expect("Could not connect to UPL service");

      let local_commits = self
        .local_commits()
        .unwrap()
        .into_iter()
        .map(|c| CommitObj {
          obj_json_string: serde_json::to_string(&c).unwrap(),
        })
        .collect::<Vec<CommitObj>>();

      let mut commits = vec![];

      for commit in local_commits {
        println!("commitobj to send {:?}", &commit);
        let mut commit = remote_client.push(commit).await.unwrap().into_inner();
        println!("{:?}", &commit);
        commits.push(commit);
      }

      println!("{:?}", commits);
    });

    Ok(())
  }
  /// Clean local repository, clear local changes
  /// And performs remote pull
  pub fn proceed_clean(&self) -> Result<(), String> {
    unimplemented!()
  }
  /// Start watcher for remote client to watch
  /// remote updates
  pub fn watch(&self) -> Result<(), String> {
    match &self.repo_details.lock().unwrap().mode {
      Mode::Remote { .. } => {}
      _ => {
        panic!(
          "Cannot start remote watch, as the repository is not in remote mode"
        )
      }
    }
    unimplemented!()
  }
  /// Merge pushed commit to remote one
  /// Returns the applied & signed remote Commit if success
  pub fn merge_pushed_commit(
    &self,
    commit_json_str: &str,
  ) -> Result<Commit, String> {
    // Lock itself
    let mut ctx = self.commit_ctx("");

    // 1) Check Commit
    // Deserialize commit object
    let mut commit: Commit = serde_json::from_str(commit_json_str)
      .map_err(|_| "Deser error during commit deser process".to_string())?;
    // Check signature
    if commit.remote_signature.is_some() {
      return Err(
        "Pushed commit has remote signature. Only local commits can be pushed!"
          .to_string(),
      );
    }

    // Check ancestor
    if let Some(latest_remote_commit_id) =
      CommitIndex::latest_remote_commit_id(&ctx)
    {
      // Only if not first commit
      if commit.ancestor_id != latest_remote_commit_id {
        // Return error if ancestor id is wrong
        return Err(
          "Commit ancestor id erro. Local repo not up-to-date. Pull required."
            .to_string(),
        );
      }
    }

    // 2) Sign all action objects

    // Deserialize action objects as universal aob
    let mut action_objects: Vec<UniversalActionObject> = vec![];
    for aob in &commit.serialized_actions {
      action_objects.push(
        serde_json::from_str(aob).map_err(|_| {
          "Error while deser aob into universal aob".to_string()
        })?,
      );
    }

    // Clear action objects
    commit.serialized_actions = vec![];

    for mut uaob in action_objects {
      // Sign action object to be a remote one
      uaob.remote_sign()?;
      // Add action object back again
      commit.add_action_object(uaob);
    }

    // 3) Check all action objects (Ancestor + Action + Signature)
    let hooks = &ctx.storage_hooks;
    for aob_str in &commit.serialized_actions {
      for hook in hooks.deref() {
        let res = hook(aob_str, CallbackMode::Check);
        if let Some(res) = res {
          if let Err(e) = res {
            return Err(e);
          }
          break;
        }
      }
    }

    // 4) ReCreate commit with signature and signed ActionObject
    commit.add_remote_signature()?;

    // 5) Add commit as remote commit
    //    merge_commit_ctx will create a merge commit context with the
    //    prepared commit, and it will auto merge into remote as merge_commit_ctx drops
    ctx.temp_commit = commit.clone();
    // 6) Return remote commit
    Ok(commit)
  }
  /// Start remote server
  pub fn serve(self) -> Result<(), String> {
    let server_addr = match &self.repo_details.lock().unwrap().mode {
      Mode::Server { server_addr } => server_addr.to_string(),
      _ => {
        panic!("Cannot start server, as the repository is not in server mode")
      }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .worker_threads(1)
      .thread_name("sync_server")
      .build()
      .unwrap();
    runtime.block_on(async {
      Server::builder()
        .add_service(ApiServer::new(self))
        .serve(server_addr.parse().unwrap())
        .await
        .expect("Error starting server");
    });
    Ok(())
  }
  // Private method to register
  // storage hooks
  // Storage update process will occur via these hooks (callbacks)
  fn add_storage_hook(
    &self,
    hook: Box<dyn Fn(&str, CallbackMode) -> Option<Result<(), String>> + Send>,
  ) -> Result<(), String> {
    self.storage_hooks.lock().unwrap().push(hook);
    Ok(())
  }
  pub fn ctx<'a>(&'a self) -> ContextGuard {
    let mutex_guard = (&self.ctx).lock().unwrap();
    ContextGuard { mutex_guard }
  }
  pub fn commit_ctx<'a>(
    &'a self,
    commit_comment: &str,
  ) -> CommitContextGuard<'a> {
    CommitContextGuard::new(self, commit_comment)
  }
  pub fn local_commits(&self) -> Result<Vec<Commit>, String> {
    CommitLog::load_locals(&self.ctx())
  }
  pub fn remote_commits(&self) -> Result<Vec<Commit>, String> {
    CommitLog::load_remotes(&self.ctx())
  }
  pub fn remote_commits_after(
    &self,
    after_id: Uuid,
  ) -> Result<Vec<Commit>, String> {
    CommitLog::load_remotes_after(&self.ctx(), after_id)
  }
}
