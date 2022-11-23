use {
  cargo_metadata::MetadataCommand, cradle::prelude::*, std::env, structopt::StructOpt,
  tempfile::tempdir,
};

#[derive(StructOpt)]
struct Arguments {
  revision: String,
  #[structopt(long)]
  publish_opuza_monero_client: bool,
}

fn main() {
  let arguments = Arguments::from_args();

  let tempdir = tempdir().unwrap();

  (
    "git",
    "clone",
    "git@github.com:refactor-ring/opuza.git",
    CurrentDir(tempdir.path()),
  )
    .run();

  env::set_current_dir(tempdir.path().join("opuza")).unwrap();

  (
    "git",
    "merge-base",
    "--is-ancestor",
    &arguments.revision,
    "master",
  )
    .run();

  ("git", "checkout", arguments.revision).run();

  let metadata = MetadataCommand::new().exec().unwrap();

  let version = metadata
    .packages
    .into_iter()
    .filter(|package| package.name == "opuza")
    .next()
    .unwrap()
    .version;

  (
    "git",
    "tag",
    "--sign",
    "--message",
    format!("Release version {}", version),
    version.to_string(),
  )
    .run();

  ("git", "push", "origin", &version.to_string()).run();

  if arguments.publish_opuza_monero_client {
    ("cargo", "publish", CurrentDir("opuza-monero-client")).run();
  }

  ("cargo", "publish").run();
}
