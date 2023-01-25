use std::{ops::Deref, path::PathBuf};

use serde::{Deserialize, Serialize};
use storage::{
  sync::{ActionExt, Context, ContextGuard, ObjectExt, Repository, Storage},
  *,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
  id: u32,
  name: String,
  age: i32,
}

impl ObjectExt for User {}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

struct AppData {
  repo: Repository,
  a: Storage<User, UserAction>,
  b: Storage<User, UserAction>,
}

impl AppData {
  fn new(
    repo: Repository,
    a: Storage<User, UserAction>,
    b: Storage<User, UserAction>,
  ) -> Self {
    Self { repo, a, b }
  }
  fn a_set_name(&self) -> Result<(), String> {
    let ctx = self.repo.ctx();
    let all = self.a.get_all(&ctx)?;
    println!("{:?}", &self.a);
    for i in all {
      let object = i.deref();
      println!("{:?}", object);
    }

    // let mut ctx = self.repo.commit_ctx("Demo commit");
    // self.a.create_object(
    //   User {
    //     id: 1,
    //     name: "Peti".into(),
    //     age: 34,
    //   },
    //   &mut ctx,
    // );

    Ok(())
  }
}

fn main() {
  // Init Demo Context
  let ctx = Context::init(PathBuf::from("./data"), "mezeipetister".into());

  // Init repo
  // let repo: Repository =
  //   Repository::init(ctx.clone(), sync::Mode::Local).unwrap();

  // Load repo
  let repo: Repository = Repository::load(ctx).unwrap();

  // Init storage
  let a: Storage<User, UserAction> =
    storage::sync::Storage::load_or_init(&repo, "demo_a".into())
      .unwrap()
      .register(&repo)
      .unwrap();

  let b: Storage<User, UserAction> =
    storage::sync::Storage::load_or_init(&repo, "demo_b".into())
      .unwrap()
      .register(&repo)
      .unwrap();

  let app_data = AppData::new(repo, a, b);

  app_data.a_set_name().unwrap();
}
