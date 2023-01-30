/// The request message containing the user's name.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HelloRequest {
  #[prost(string, tag = "1")]
  pub name: ::prost::alloc::string::String,
}
/// The response message containing the greetings
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HelloReply {
  #[prost(string, tag = "1")]
  pub message: ::prost::alloc::string::String,
}
#[doc = r" Generated client implementations."]
pub mod api_client {
  #![allow(unused_variables, dead_code, missing_docs)]
  use tonic::codegen::*;
  pub struct ApiClient<T> {
    inner: tonic::client::Grpc<T>,
  }
  impl ApiClient<tonic::transport::Channel> {
    #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
    pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
      D: std::convert::TryInto<tonic::transport::Endpoint>,
      D::Error: Into<StdError>,
    {
      let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
      Ok(Self::new(conn))
    }
  }
  impl<T> ApiClient<T>
  where
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::ResponseBody: Body + HttpBody + Send + 'static,
    T::Error: Into<StdError>,
    <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
  {
    pub fn new(inner: T) -> Self {
      let inner = tonic::client::Grpc::new(inner);
      Self { inner }
    }
    pub fn with_interceptor(
      inner: T,
      interceptor: impl Into<tonic::Interceptor>,
    ) -> Self {
      let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
      Self { inner }
    }
    pub async fn clone(
      &mut self,
      request: impl tonic::IntoRequest<super::HelloRequest>,
    ) -> Result<tonic::Response<super::HelloReply>, tonic::Status> {
      self.inner.ready().await.map_err(|e| {
        tonic::Status::new(
          tonic::Code::Unknown,
          format!("Service was not ready: {}", e.into()),
        )
      })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static("/sync_api.Api/Clone");
      self.inner.unary(request.into_request(), path, codec).await
    }
    pub async fn pull(
      &mut self,
      request: impl tonic::IntoRequest<super::HelloRequest>,
    ) -> Result<tonic::Response<super::HelloReply>, tonic::Status> {
      self.inner.ready().await.map_err(|e| {
        tonic::Status::new(
          tonic::Code::Unknown,
          format!("Service was not ready: {}", e.into()),
        )
      })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static("/sync_api.Api/Pull");
      self.inner.unary(request.into_request(), path, codec).await
    }
    pub async fn push(
      &mut self,
      request: impl tonic::IntoRequest<super::HelloRequest>,
    ) -> Result<tonic::Response<super::HelloReply>, tonic::Status> {
      self.inner.ready().await.map_err(|e| {
        tonic::Status::new(
          tonic::Code::Unknown,
          format!("Service was not ready: {}", e.into()),
        )
      })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static("/sync_api.Api/Push");
      self.inner.unary(request.into_request(), path, codec).await
    }
    pub async fn watch(
      &mut self,
      request: impl tonic::IntoRequest<super::HelloRequest>,
    ) -> Result<
      tonic::Response<tonic::codec::Streaming<super::HelloReply>>,
      tonic::Status,
    > {
      self.inner.ready().await.map_err(|e| {
        tonic::Status::new(
          tonic::Code::Unknown,
          format!("Service was not ready: {}", e.into()),
        )
      })?;
      let codec = tonic::codec::ProstCodec::default();
      let path = http::uri::PathAndQuery::from_static("/sync_api.Api/Watch");
      self
        .inner
        .server_streaming(request.into_request(), path, codec)
        .await
    }
  }
  impl<T: Clone> Clone for ApiClient<T> {
    fn clone(&self) -> Self {
      Self {
        inner: self.inner.clone(),
      }
    }
  }
  impl<T> std::fmt::Debug for ApiClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "ApiClient {{ ... }}")
    }
  }
}
#[doc = r" Generated server implementations."]
pub mod api_server {
  #![allow(unused_variables, dead_code, missing_docs)]
  use tonic::codegen::*;
  #[doc = "Generated trait containing gRPC methods that should be implemented for use with ApiServer."]
  #[async_trait]
  pub trait Api: Send + Sync + 'static {
    async fn clone(
      &self,
      request: tonic::Request<super::HelloRequest>,
    ) -> Result<tonic::Response<super::HelloReply>, tonic::Status>;
    async fn pull(
      &self,
      request: tonic::Request<super::HelloRequest>,
    ) -> Result<tonic::Response<super::HelloReply>, tonic::Status>;
    async fn push(
      &self,
      request: tonic::Request<super::HelloRequest>,
    ) -> Result<tonic::Response<super::HelloReply>, tonic::Status>;
    #[doc = "Server streaming response type for the Watch method."]
    type WatchStream: futures_core::Stream<Item = Result<super::HelloReply, tonic::Status>>
      + Send
      + Sync
      + 'static;
    async fn watch(
      &self,
      request: tonic::Request<super::HelloRequest>,
    ) -> Result<tonic::Response<Self::WatchStream>, tonic::Status>;
  }
  #[derive(Debug)]
  pub struct ApiServer<T: Api> {
    inner: _Inner<T>,
  }
  struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
  impl<T: Api> ApiServer<T> {
    pub fn new(inner: T) -> Self {
      let inner = Arc::new(inner);
      let inner = _Inner(inner, None);
      Self { inner }
    }
    pub fn with_interceptor(
      inner: T,
      interceptor: impl Into<tonic::Interceptor>,
    ) -> Self {
      let inner = Arc::new(inner);
      let inner = _Inner(inner, Some(interceptor.into()));
      Self { inner }
    }
  }
  impl<T, B> Service<http::Request<B>> for ApiServer<T>
  where
    T: Api,
    B: HttpBody + Send + Sync + 'static,
    B::Error: Into<StdError> + Send + 'static,
  {
    type Response = http::Response<tonic::body::BoxBody>;
    type Error = Never;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn poll_ready(
      &mut self,
      _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
      Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: http::Request<B>) -> Self::Future {
      let inner = self.inner.clone();
      match req.uri().path() {
        "/sync_api.Api/Clone" => {
          #[allow(non_camel_case_types)]
          struct CloneSvc<T: Api>(pub Arc<T>);
          impl<T: Api> tonic::server::UnaryService<super::HelloRequest> for CloneSvc<T> {
            type Response = super::HelloReply;
            type Future =
              BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
            fn call(
              &mut self,
              request: tonic::Request<super::HelloRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move { (*inner).clone(request).await };
              Box::pin(fut)
            }
          }
          let inner = self.inner.clone();
          let fut = async move {
            let interceptor = inner.1.clone();
            let inner = inner.0;
            let method = CloneSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = if let Some(interceptor) = interceptor {
              tonic::server::Grpc::with_interceptor(codec, interceptor)
            } else {
              tonic::server::Grpc::new(codec)
            };
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/sync_api.Api/Pull" => {
          #[allow(non_camel_case_types)]
          struct PullSvc<T: Api>(pub Arc<T>);
          impl<T: Api> tonic::server::UnaryService<super::HelloRequest> for PullSvc<T> {
            type Response = super::HelloReply;
            type Future =
              BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
            fn call(
              &mut self,
              request: tonic::Request<super::HelloRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move { (*inner).pull(request).await };
              Box::pin(fut)
            }
          }
          let inner = self.inner.clone();
          let fut = async move {
            let interceptor = inner.1.clone();
            let inner = inner.0;
            let method = PullSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = if let Some(interceptor) = interceptor {
              tonic::server::Grpc::with_interceptor(codec, interceptor)
            } else {
              tonic::server::Grpc::new(codec)
            };
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/sync_api.Api/Push" => {
          #[allow(non_camel_case_types)]
          struct PushSvc<T: Api>(pub Arc<T>);
          impl<T: Api> tonic::server::UnaryService<super::HelloRequest> for PushSvc<T> {
            type Response = super::HelloReply;
            type Future =
              BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
            fn call(
              &mut self,
              request: tonic::Request<super::HelloRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move { (*inner).push(request).await };
              Box::pin(fut)
            }
          }
          let inner = self.inner.clone();
          let fut = async move {
            let interceptor = inner.1.clone();
            let inner = inner.0;
            let method = PushSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = if let Some(interceptor) = interceptor {
              tonic::server::Grpc::with_interceptor(codec, interceptor)
            } else {
              tonic::server::Grpc::new(codec)
            };
            let res = grpc.unary(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        "/sync_api.Api/Watch" => {
          #[allow(non_camel_case_types)]
          struct WatchSvc<T: Api>(pub Arc<T>);
          impl<T: Api>
            tonic::server::ServerStreamingService<super::HelloRequest>
            for WatchSvc<T>
          {
            type Response = super::HelloReply;
            type ResponseStream = T::WatchStream;
            type Future =
              BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
            fn call(
              &mut self,
              request: tonic::Request<super::HelloRequest>,
            ) -> Self::Future {
              let inner = self.0.clone();
              let fut = async move { (*inner).watch(request).await };
              Box::pin(fut)
            }
          }
          let inner = self.inner.clone();
          let fut = async move {
            let interceptor = inner.1;
            let inner = inner.0;
            let method = WatchSvc(inner);
            let codec = tonic::codec::ProstCodec::default();
            let mut grpc = if let Some(interceptor) = interceptor {
              tonic::server::Grpc::with_interceptor(codec, interceptor)
            } else {
              tonic::server::Grpc::new(codec)
            };
            let res = grpc.server_streaming(method, req).await;
            Ok(res)
          };
          Box::pin(fut)
        }
        _ => Box::pin(async move {
          Ok(
            http::Response::builder()
              .status(200)
              .header("grpc-status", "12")
              .header("content-type", "application/grpc")
              .body(tonic::body::BoxBody::empty())
              .unwrap(),
          )
        }),
      }
    }
  }
  impl<T: Api> Clone for ApiServer<T> {
    fn clone(&self) -> Self {
      let inner = self.inner.clone();
      Self { inner }
    }
  }
  impl<T: Api> Clone for _Inner<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone(), self.1.clone())
    }
  }
  impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "{:?}", self.0)
    }
  }
  impl<T: Api> tonic::transport::NamedService for ApiServer<T> {
    const NAME: &'static str = "sync_api.Api";
  }
}
