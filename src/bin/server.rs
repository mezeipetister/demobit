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

  fn display(&self) -> String {
    match &self {
      UserAction::SetName(n) => format!("SetName to {}", n),
      UserAction::SetAge(a) => format!("SetAge to {}", a),
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

  fn a_get_by_id(&self) -> Result<(), String> {
    let ctx = self.repo.ctx();
    let res = self
      .a
      .get_by_filter(&ctx, |i| i.id == 1)?
      .first()
      .unwrap()
      .deref()
      .to_owned();
    println!("{:?}", res);
    Ok(())
  }

  fn a_get_all(&self) -> Result<(), String> {
    let ctx = self.repo.ctx();
    let all = self.a.get_all(&ctx)?;
    for i in all {
      let object = i.deref();
      println!("{:?}", object);
    }
    Ok(())
  }

  fn a_get_age(&self, id: u32) -> Result<i32, String> {
    let ctx = self.repo.ctx();
    self
      .a
      .get_first_by_filter(&ctx, |i| i.id == id)
      .map(|i| i.age)
  }

  fn a_create(&self, id: u32) -> Result<(), String> {
    let mut ctx = self.repo.commit_ctx("Demo commit");
    self.a.create_object(
      User {
        id,
        name: "Peti".into(),
        age: 34,
      },
      &mut ctx,
    );

    Ok(())
  }

  fn a_set_age(&self, id: u32, age: i32) -> Result<(), String> {
    // let ctx = self.repo.ctx();
    let mut ctx = self.repo.commit_ctx("Demo commit");
    self.a.patch_by_filter(
      &mut ctx,
      |i| i.id == id,
      UserAction::SetAge(age),
    )?;
    Ok(())
  }
}

fn main() {
  pretty_env_logger::init();

  // Init Demo Context
  let ctx =
    Context::init(PathBuf::from("./data/server"), "mezeipetister".into());

  // Init repo
  let repo: Repository = Repository::init(
    ctx.clone(),
    sync::Mode::Server {
      server_addr: "[::1]:50059".to_string(),
    },
  )
  .unwrap();

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

  // let app_data = AppData::new(repo, a, b);

  // return;

  // Load repo
  // let repo: Repository = Repository::load(ctx).unwrap();

  repo.serve().unwrap();
}
