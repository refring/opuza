use clap::Parser;
use regex::Regex;

#[derive(Parser)]
struct Arguments {
  #[arg(long)]
  reference: String,
}

fn main() {
  let arguments = Arguments::parse();

  let regex = Regex::new("^refs/tags/[[:digit:]]+[.][[:digit:]]+[.][[:digit:]]+$")
    .expect("Failed to compile release regex");

  println!(
    "::set-output name=value::{}",
    !regex.is_match(&arguments.reference)
  );
}
