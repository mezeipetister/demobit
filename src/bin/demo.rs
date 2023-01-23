use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use storage::{
  sync::{ActionExt, ContextGuard, ObjectExt, Repository, Storage},
  *,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
  id: u32,
  name: String,
  age: i32,
}

impl ObjectExt for User {}

#[derive(Serialize, Deserialize, Clone)]
enum UserAction {
  SetName(String),
  SetAge(i32),
}

impl ActionExt for UserAction {
  type ObjectType = User;

  fn apply_patch(
    &self,
    object: &Self::ObjectType,
    dtime: chrono::DateTime<chrono::Utc>,
    uid: &str,
  ) -> Result<Self::ObjectType, String> {
    match self {
      UserAction::SetName(name) => {
        let mut o = object.clone();
        o.name = name.clone();
        return Ok(o);
      }
      UserAction::SetAge(age) => {
        let mut o = object.clone();
        o.age = *age;
        return Ok(o);
      }
    }
  }
}

fn main() {
  // Demo Context
  let ctx = ContextGuard::new(PathBuf::from("./data"), "mezeipetister".into());

  // Init repo
  // let repo: Repository =
  //   Repository::init(ctx.clone(), sync::Mode::Local).unwrap();

  // Load repo
  let repo: Repository = Repository::load(ctx).unwrap();
  let ctx = repo.

  // Init storage
  let a: Storage<User, UserAction> =
    storage::sync::Storage::load_or_init(&ctx, "demo".into())
      .unwrap()
      .register(ctx, &repo)
      .unwrap();
}
