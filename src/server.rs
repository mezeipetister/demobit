use crate::sync::Repository;
use async_stream::stream;
use futures::pin_mut;
use futures_util::stream::StreamExt;
use std::pin::Pin;
use sync_api::api_server::{Api, ApiServer};
use sync_api::{CommitObj, PullRequest};
use tokio_stream::wrappers::ReceiverStream;
use tonic::codegen::futures_core::Stream;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

pub mod sync_api {
  tonic::include_proto!("sync_api");
}

#[tonic::async_trait]
impl Api for Repository {
  type PullStream = ReceiverStream<Result<CommitObj, Status>>;

  async fn pull(
    &self,
    request: Request<PullRequest>, // Accept request of type HelloRequest
  ) -> Result<Response<Self::PullStream>, Status> {
    // Return an instance of type HelloReply
    let (mut tx, rx) = tokio::sync::mpsc::channel(100);

    // Get resources as Vec<SourceObject>
    let commit_id_str = &request.into_inner().after_commit_id;

    let res = match commit_id_str.len() > 0 {
      true => {
        let after_id = Uuid::parse_str(commit_id_str)
          .map_err(|_| Status::invalid_argument("Wrong commit_id format"))?;
        self.remote_commits_after(after_id).map_err(|_| {
          Status::invalid_argument("Error collection remote logs")
        })?
      }
      false => self.remote_commits().map_err(|_| {
        Status::invalid_argument("Error collecting remote logs")
      })?,
    };

    // Send the result items through the channel
    tokio::spawn(async move {
      for commit in res.into_iter() {
        let r: CommitObj = CommitObj {
          obj_json_string: serde_json::to_string(&commit).unwrap(),
        };
        tx.send(Ok(r)).await.unwrap();
      }
    });

    // Send back the receiver
    Ok(Response::new(ReceiverStream::new(rx)))
  }

  type PushStream = ReceiverStream<Result<CommitObj, Status>>;

  async fn push(
    &self,
    request: Request<tonic::Streaming<CommitObj>>, // Accept request of type HelloRequest
  ) -> Result<Response<Self::PushStream>, Status> {
    let mut stream = request.into_inner();

    let s = stream! {
        while let Some(new_commit) = stream.next().await {
          if let Ok(commit_obj) = new_commit {
            if let Ok(res) = self.merge_pushed_commit(&commit_obj.obj_json_string) {
              yield res;
            }
          }
        }
    };

    pin_mut!(s);

    let (mut tx, rx) = tokio::sync::mpsc::channel(100);

    while let Some(value) = s.next().await {
      let res = CommitObj {
        obj_json_string: serde_json::to_string(&value).unwrap(),
      };
      tx.send(Ok(res)).await.unwrap();
    }

    // Send back the receiver
    Ok(Response::new(ReceiverStream::new(rx)))
  }
}
