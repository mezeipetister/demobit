fn main() {
  tonic_build::compile_protos("./sync_api.proto").unwrap();
}
