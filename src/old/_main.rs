use chrono::{DateTime, Utc};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

mod v1 {
    use chrono::{DateTime, Utc};
    use schemars::{schema_for, JsonSchema};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, JsonSchema)]
    pub struct DataObject {
        object: String,
        actions: Vec<Action>,
    }

    #[derive(Serialize, Deserialize, JsonSchema)]
    pub enum Action {
        Add { id: String, name: String },
        Remove { id: String },
    }
}

mod v2 {
    use chrono::{DateTime, Utc};
    use schemars::{schema_for, JsonSchema};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, JsonSchema)]
    pub struct DataObject {
        object: String,
        actions: Vec<Action>,
    }

    #[derive(Serialize, Deserialize, JsonSchema)]
    pub enum Action {
        Add { id: String, name: String },
        Remove { id: String },
        Nothing,
    }
}

fn main() {
    // let object = crate::v1::DataObject {
    //     object: "".into(),
    //     actions: vec![crate::v1::Action::Add {
    //         id: "1".into(),
    //         name: "Demo".into(),
    //     }],
    // };

    let schema = schema_for!(crate::v1::DataObject);
    let schema2 = schema_for!(crate::v2::DataObject);
    let res = schema == schema2;
    println!("{}", res);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
