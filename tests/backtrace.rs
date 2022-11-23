use {opuza_test_context::OpuzaTestContext, reqwest::StatusCode};

#[test]
fn errors_contain_backtraces() {
  let context = OpuzaTestContext::builder().backtraces(true).build();
  assert_eq!(context.status("files/.hidden"), StatusCode::NOT_FOUND);
  let stderr = context.kill();
  assert!(stderr.contains("opuza::vfs::Vfs::check_path"));
}
