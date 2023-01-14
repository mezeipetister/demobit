use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub struct Repository {
    remote: (),  // Remote log
    staging: (), // Staging log
}

pub struct Storage<
    T: Serialize + for<'de> Deserialize<'de> + Default + Clone + Sized + JsonSchema,
    A: ActionExt,
> {
    remote: Vec<StorageObject<T, A>>,
    staging: (),
    local: (),
}

pub enum Action<A: ActionExt> {
    Create { init_json_object: String },
    Custom(A),
}

pub struct ActionObject<A: ActionExt> {
    uid: String,
    dtime: DateTime<Utc>,
    action: Action<A>,
}

pub trait ActionExt {
    type StorageType;
    fn apply(db: &Self::StorageType) -> Result<(), String>;
}

pub struct Commit {}

pub struct StorageObject<
    T: Serialize + for<'de> Deserialize<'de> + Default + Clone + Sized + JsonSchema,
    A: ActionExt,
> {
    object_json: T,
    initial_object_json: T,
    actions: Vec<ActionObject<A>>,
}
