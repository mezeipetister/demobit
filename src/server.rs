use std::pin::Pin;

use sync_api::api_server::{Api, ApiServer};
use sync_api::{CommitObj, PullRequest};
use tokio_stream::wrappers::ReceiverStream;
use tonic::codegen::futures_core::Stream;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

use crate::sync::Repository;

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
    let after_id =
      Uuid::parse_str(&request.into_inner().after_commit_id).unwrap();
    let res = self.remote_commits_after(after_id).unwrap();

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

  type PushStream =
    Pin<Box<dyn Stream<Item = Result<CommitObj, Status>> + Send>>;

  async fn push(
    &self,
    request: Request<tonic::Streaming<CommitObj>>, // Accept request of type HelloRequest
  ) -> Result<Response<Self::PushStream>, Status> {
    // Return an instance of type HelloReply
    unimplemented!()
  }
}
