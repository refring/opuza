use clap::builder::ArgPredicate;
use clap::crate_version;
use clap::ColorChoice;
use clap::Parser;
use {crate::common::*, clap::ArgGroup};

#[derive(Debug, Parser)]
#[command(
  group = ArgGroup::new("port").multiple(true).required(true),
  color = if cfg!(test) { ColorChoice::Never } else { ColorChoice::Auto },
  version = crate_version!())
]
pub(crate) struct Arguments {
  #[arg(
    long,
    help = "Store TLS certificates fetched from Let's Encrypt via the ACME protocol in <acme-cache-directory>."
  )]
  pub(crate) acme_cache_directory: Option<PathBuf>,
  #[arg(
    long,
    help = "Request TLS certificate for <acme-domain>. This opuza instance must be reachable at <acme-domain>:443 to respond to Let's Encrypt ACME challenges."
  )]
  pub(crate) acme_domain: Vec<String>,
  #[arg(
    long,
    default_value = "0.0.0.0",
    help = "Listen on <address> for incoming requests."
  )]
  pub(crate) address: String,
  #[arg(long, help = "Serve files from <directory>")]
  pub(crate) directory: PathBuf,
  #[arg(
    long,
    group = "port",
    help = "Listen on <http-port> for incoming HTTP requests."
  )]
  pub(crate) http_port: Option<u16>,
  #[arg(
    long,
    group = "port",
    help = "Listen on <https-port> for incoming HTTPS requests.",
    requires_ifs = [(ArgPredicate::IsPresent, "acme_cache_directory"), (ArgPredicate::IsPresent,"acme_domain")]
  )]
  pub(crate) https_port: Option<u16>,
  #[arg(
    long,
    help = "Redirect HTTP requests on <https-redirect-port> to HTTPS on <https-port>.",
    requires = "https_port"
  )]
  pub(crate) https_redirect_port: Option<u16>,
  #[arg(
    long,
    help = "Connect to LND gRPC server with host and port <lnd-rpc-authority>. By default a locally running LND instance will expose its gRPC API on `localhost:10009`."
  )]
  pub(crate) _lnd_rpc_authority: Option<Authority>,
  #[arg(
    long,
    help = "Read LND's TLS certificate from <lnd-rpc-cert-path>. Needed if LND uses a self-signed certificate. By default LND writes its TLS certificate to `~/.lnd/tls.cert`.",
    requires = "_lnd_rpc_authority"
  )]
  pub(crate) _lnd_rpc_cert_path: Option<PathBuf>,
  #[arg(
    long,
    help = "Read LND gRPC macaroon from <lnd-rpc-macaroon-path>. Needed if LND requires macaroon authentication. The macaroon must include permissions for creating and querying invoices. By default LND writes its invoice macaroon to `~/.lnd/data/chain/bitcoin/mainnet/invoice.macaroon`.",
    requires = "_lnd_rpc_authority"
  )]
  pub(crate) _lnd_rpc_macaroon_path: Option<PathBuf>,
  #[arg(
    long,
    help = "Connect to monero node rpc.",
  )]
  pub(crate) monero_rpc_address: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::assert_contains;
  use unindent::Unindent;

  #[test]
  fn https_redirect_port_requires_https_port() {
    assert_contains(
      &Arguments::try_parse_from([
        "opuza",
        "--directory=www",
        "--https-redirect-port=0",
        "--http-port=0",
      ])
      .unwrap_err()
      .to_string(),
      &"
        the following required arguments were not provided:
          --acme-cache-directory <ACME_CACHE_DIRECTORY>
          --acme-domain <ACME_DOMAIN>
          --https-port <HTTPS_PORT>
      "
      .unindent(),
    );
  }

  #[test]
  fn https_port_requires_acme_cache_directory() {
    assert_contains(
      &Arguments::try_parse_from(["opuza", "--directory=www", "--https-port=0"])
        .unwrap_err()
        .to_string(),
      &"
        the following required arguments were not provided:
          --acme-cache-directory <ACME_CACHE_DIRECTORY>
      "
      .unindent(),
    );
  }

  #[test]
  fn https_port_requires_acme_domain() {
    assert_contains(
      &Arguments::try_parse_from([
        "opuza",
        "--directory=www",
        "--https-port=0",
        "--acme-cache-directory=cache",
      ])
      .unwrap_err()
      .to_string(),
      &"
        the following required arguments were not provided:
          --acme-domain <ACME_DOMAIN>
      "
      .unindent(),
    );
  }

  #[test]
  fn require_at_least_one_port_argument() {
    assert_contains(
      &Arguments::try_parse_from(["opuza", "--directory=www"])
        .unwrap_err()
        .to_string(),
      &"
      the following required arguments were not provided:
        <--http-port <HTTP_PORT>|--https-port <HTTPS_PORT>>
      "
      .unindent(),
    );
  }
}
