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

use crate::{
  fs::{binary_init, binary_read, binary_update},
  prelude::{path_helper, sha1_signature},
};

/// Storage Context
/// containing operational details
/// such as db root path or uid
#[derive(Clone)]
pub struct Context {
  pub db_root_path: PathBuf,
  pub uid: String,
}

impl Context {
  pub fn new(db_root_path: PathBuf, uid: String) -> Self {
    Self { db_root_path, uid }
  }
}

/// Action trait for Actionable types
/// Implemented types can be used as storage patch objects.
pub trait ActionExt: Clone {
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

pub trait ObjectExt: Debug + Clone {}

/// Generic acion representation
/// Atomic action kinds with the following states:
/// Create, Patch, Remove, Recover
#[derive(Serialize, Deserialize, Clone)]
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
#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize)]
pub struct Commit {
  id: Uuid,
  uid: String,
  dtime: DateTime<Utc>,
  comment: String,
  ancestor_id: Uuid,
  serialized_actions: Vec<String>, // ActionObject JSONs in Vec
}

#[derive(Serialize, Deserialize)]
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
  A: ActionExt<ObjectType = T> + Serialize + for<'de> Deserialize<'de>,
{
  fn new_from_aob(aob: ActionObject<T, A>) -> Result<Self, String> {
    if let ActionKind::Create(data) = aob.action.clone() {
      let res = match aob.is_local() {
        true => Self {
          id: aob.id,
          storage_id: aob.storage_id.clone(),
          remote_actions: vec![],
          local_actions: vec![aob],
          remote_object: None,
          local_object: data,
        },
        false => Self {
          id: aob.id,
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
      storage_id: self.storage_id.clone(),
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
  // Add action object
  fn add_action_object(
    &mut self,
    action_object: ActionObject<T, A>,
    ctx: &Context,
  ) -> Result<&T, String> {
    if action_object.is_local() {
      return self.add_local_action_object(action_object, ctx);
    } else {
      return self.add_remote_action_object(action_object, ctx);
    }
  }
  // Add local action object to Storage Object
  fn add_local_action_object(
    &mut self,
    action_object: ActionObject<T, A>,
    ctx: &Context,
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
      // Save to fs
      self.save_to_fs(ctx)?;
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
    ctx: &Context,
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
      // Save to FS
      self.save_to_fs(ctx)?;
      // Return current local object
      // Important! We return LOCAL, as its the latest version of our
      // data object.
      return Ok(&self.local_object);
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
#[derive(Clone)]
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

#[derive(Serialize, Deserialize)]
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
  A:
    ActionExt<ObjectType = T> + Serialize + for<'de> Deserialize<'de> + 'static,
{
  /// Init a storage by providing a repository object
  /// Based on its data it can pull itself, or init itself
  /// as a local repository with initial data
  pub fn load_or_init(
    ctx: &Context,
    storage_id: String,
  ) -> Result<Self, String> {
    let storage_details_path =
      path_helper::storage_details_path(ctx, &storage_id);
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

  // Get a single storage object by id
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

  // Add action object to storage object
  pub fn add_action_object(
    &self,
    ctx: &Context,
    action_object: ActionObject<T, A>,
  ) -> Result<T, String> {
    let object_id = action_object.object_id;
    // Create a new one
    let data = match action_object.is_kind_create() {
      true => {
        // Create new storage object
        let new_storage_object = StorageObject::new_from_aob(action_object)?;
        // Get data
        let data = new_storage_object.local_object.clone();
        // Get Object path
        let path = path_helper::storage_object_path(
          ctx,
          &new_storage_object.storage_id,
          new_storage_object.id,
        );
        // Init in FS and save its content as binary
        binary_init(path, new_storage_object)?;
        // Return data
        data
      }
      // Try to patch existing one
      false => self
        .get_object_by_id(ctx, object_id)?
        .add_action_object(action_object, ctx)?
        .clone(),
    };
    Ok(data)
  }

  /// Register a callback to a given repository
  /// Repository will use this callback to update storage
  pub fn register<'a: 'static>(
    &'a self,
    ctx: Context,
    repository: &Repository,
  ) -> Result<(), String> {
    let _self = self.clone();
    repository.add_storage_hook(Box::new(
      move |aobstr: &str| -> Option<Result<(), String>> {
        // Try to deserialize action object
        if let Ok(aob) = serde_json::from_str::<ActionObject<T, A>>(aobstr) {
          // Check if storage target is ok
          if &aob.storage_id != &self.storage_id() {
            return None;
          }
          match self.add_action_object(&ctx, aob) {
            Ok(_) => return Some(Ok(())),
            Err(e) => return Some(Err(e)),
          };
        }
        None
      },
    ))?;
    Ok(())
  }
}

// Repository Mode
// Local, Remote or Server
#[derive(Serialize, Deserialize)]
enum Mode {
  Server { port_number: usize },
  Remote { remote_url: String },
  Local,
}

/// Commit Log
/// contains all the repository related logs
#[derive(Default, Serialize, Deserialize)]
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
  fn init(ctx: &Context) -> Result<(), String> {
    binary_init(path_helper::commit_log(ctx), Self::default())?;
    Ok(())
  }
  fn load(ctx: &Context) -> Result<Self, String> {
    binary_read(path_helper::commit_log(ctx))
  }
}

#[derive(Serialize, Deserialize)]
struct RepoDetails {
  mode: Mode,
}

impl RepoDetails {
  fn init(ctx: &Context, mode: Mode) -> Result<(), String> {
    binary_init(path_helper::repo_details(ctx), RepoDetails { mode })?;
    Ok(())
  }
}

pub struct RepoData {
  ctx: Context,
  commit_log: CommitLog,
  repo_details: RepoDetails,
  storage_hooks: Vec<Box<dyn Fn(&str) -> Option<Result<(), String>>>>,
}

impl RepoData {
  fn add_storage_hook(
    &mut self,
    hook: Box<dyn Fn(&str) -> Option<Result<(), String>>>,
  ) -> Result<(), String> {
    self.storage_hooks.push(hook);
    Ok(())
  }
}

pub struct Repository {
  inner: Arc<Mutex<RepoData>>,
}

impl Repository {
  /// Init local repository
  /// No remote server, no pull/push
  /// Only local commits
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
  /// Init remote repository by
  /// syncing its remote data to local
  /// Pull push sync is required
  pub fn init_remote(ctx: Context) -> Result<Self, String> {
    unimplemented!()
  }
  /// Start remote repository
  /// and watch for client pull/push sync requests
  pub fn start_server(self) -> Result<(), String> {
    unimplemented!()
  }
  // Private method to register
  // storage hooks
  // Storage update process will occur via these hooks (callbacks)
  fn add_storage_hook(
    &self,
    hook: Box<dyn Fn(&str) -> Option<Result<(), String>>>,
  ) -> Result<(), String> {
    self.inner.lock().unwrap().add_storage_hook(hook)
  }
}
