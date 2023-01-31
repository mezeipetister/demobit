use std::pin::Pin;

use tonic::codegen::futures_core::Stream;
use tonic::{transport::Server, Request, Response, Status};

use sync_api::api_server::{Api, ApiServer};
use sync_api::{CommitObj, PullRequest};

use crate::sync::Repository;

pub mod sync_api {
  tonic::include_proto!("sync_api");
}

#[tonic::async_trait]
impl Api for Repository {
  type PullStream =
    Pin<Box<dyn Stream<Item = Result<CommitObj, Status>> + Send>>;

  async fn pull(
    &self,
    request: Request<PullRequest>, // Accept request of type HelloRequest
  ) -> Result<Response<Self::PullStream>, Status> {
    // Return an instance of type HelloReply
    unimplemented!()
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
