fn main() {
  // tonic_build::compile_protos("./proto/api.proto").unwrap();
  tonic_build::configure()
    // .build_server(false)
    .out_dir("src") // you can change the generated code's location
    .compile(
      &["./proto/api.proto"],
      &["./proto"], // specify the root location to search proto dependencies
    )
    .unwrap();
}
