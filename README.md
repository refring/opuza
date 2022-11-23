<h1 align="center">Opuza</h1>
<br />
<div align="center">
  <a href="https://crates.io/crates/opuza"><img src="https://img.shields.io/crates/v/opuza.svg?logo=rust" alt="crate"/></a>
  <a href="https://github.com/refactor-ring/opuza/actions"><img src="https://github.com/refactor-ring/opuza/workflows/Build/badge.svg" alt="build" /></a>
  <a href="https://t.me/opuzadiscussion"><img src="https://img.shields.io/static/v1?label=chat&message=Telegram&color=blue&logo=telegram" alt="chat on telegram"/></a>
</div>
<br />

`opuza` serves the contents of a local directory, providing file listings and downloads over HTTP.
For example, you can point it at a directory full of PDFs, allowing users to browse and view the PDFs in their web browser.
If `opuza` is connected to [monero-wallet-rpc](https://www.getmonero.org/resources/developer-guides/wallet-rpc.html), it can be configured to require [Monero](https://web.getmonero.org/) payments for downloads.

Public Opuza instances:
- Open an issue or submit a PR if you run an Opuza instance and would like it to appear in this readme!

Opuza is free software developed by [@refactor-ring](https://github.com/refactor-ring/), forked of the great project [Agora](https://github.com/agora-org/agora)

## Support, Feedback, and Discussion

If you have a question, want to request a feature, or find a bug, please feel free to [open an issue](https://github.com/refactor-ring/opuza/issues/new) or [join our Telegram group](https://t.me/opuzadiscussion).

The developer can also be reached [via email](mailto:refactor_ring0@proton.me?subject=Opuza).

## Running

```bash
$ mkdir files
$ echo 'amazing content' > files/file.txt
$ opuza --directory files --http-port 1234
$ curl http://localhost:1234/files/file.txt
```

See `opuza --help` for more configuration options.

## Installation

Pre-built binaries for Linux, MacOS, and Windows can be found on [the releases page](https://github.com/refactor-ring/opuza/releases).

## Building from Source

`opuza` is written in [Rust](https://www.rust-lang.org/) and built with `cargo`.
You can install Rust with [rustup](https://rustup.rs/).

Inside the checked out repository, running `cargo build --release` will build `opuza` and copy the binary to `./target/release/opuza`.

From within the repository, you can also run, e.g., `cargo install --locked --path . --root /usr/local`, which will copy `opuza` to `/usr/local/bin/opuza`.

## Running with Docker

The `opuza` Docker image can be pulled from [ghcr](/../../pkgs/container/opuza).


### Building Opuza Docker Image

The Docker image can also be built directly from within the repository.

Building the image:
```bash
docker build --tag opuza:latest .
```

### Running Opuza in Docker

The Docker image can used to serve files from your host machine, and connect to your existing monero-wallet-rpc instance.

To run `opuza` with a local directory `~/my-files`:
```bash
docker run \
  --network="host" \
  -e FILES_DIR=/files \
  -e OPUZA_PORT=8080 \
  -v ~/my-files:/files \
  opuza:latest
```

## Releases Notifications

To receive release notifications on GitHub, you can watch this repository with [custom notification settings](https://docs.github.com/en/github/managing-subscriptions-and-notifications-on-github/setting-up-notifications/configuring-notifications#configuring-your-watch-settings-for-an-individual-repository).

Additionally, an [RSS](https://en.wikipedia.org/wiki/RSS) feed of `opuza` releases is published [here](https://github.com/refactor-ring/opuza/releases.atom).

## Deployment

The `opuza` binary contains its static assets, so it can be copied and run from anywhere on the filesystem.
By default `cargo` links to system libraries dynamically.
You can avoid this by using the `x86_64-unknown-linux-musl` target: `cargo build --target=x86_64-unknown-linux-musl --release`.
This produces a statically linked binary that runs on, e.g., Alpine and CentOS Linux.

### Configuration

You can configure the network port and address `opuza` listens on, and the directory it serves.
See `opuza --help` for details.

### HTTPS Configuration

If you're running `opuza` on a public domain it can be configured to automatically request TLS certificates for HTTPS from [Let's Encrypt](https://letsencrypt.org/) via the [ACME](https://datatracker.ietf.org/doc/html/rfc8555) protocol.
See the `--acme-*` and `--https-*` flags in `opuza --help` for details.

### Monero-wallet-rpc Configuration

By default `opuza` serves files for free.
To charge for downloads, `opuza` must be connected to an [monero-wallet-rpc](https://www.getmonero.org/resources/developer-guides/wallet-rpc.html) instance.
There are multiple command line flags to configure this connection, see `opuza --help` for details.

To configure which files are free and which are paid, see [Access Configuration](#access-configuration) below.

### Access Configuration

You can put a `.opuza.yaml` configuration file into directories served by `opuza` to configure access to files in that directory.

An example configuration is:

```yaml
# whether or not to charge for files
paid: true
# price for files in XMR
base-price: 0.1 XMR
```

Access configuration applies recursively to files in subdirectories.
For example you can put this configuration in your base directory:

```yaml
paid: false
base-price: 0.01 XMR
```

Then in some subdirectories you can charge for file downloads by creating an `subdir/.opuza.yaml` like this:

```yaml
paid: true
```

The default configuration is:

```yaml
paid: false
# `base-price` does not have a default. Setting `paid` to `true`
# while not having a `base-price` causes an error.
base-price: null
```

### Custom Index Pages

`opuza` serves directory file listings.
If a `.index.md` file is present in a directory, `opuza` will render the contained Markdown as HTML and include it with the file listing. `opuza` expects Commonmark Markdown, extended with footnotes, [strikethrough](https://github.github.com/gfm/#strikethrough-extension-), [tables](https://github.github.com/gfm/#tables-extension-), and [task lists](https://github.github.com/gfm/#task-list-items-extension-).

## Buying Files from an Opuza Instance

You can navigate to any Opuza instance and browse the hosted files.
Opuza instances can host a mix of free and paid files.
For paid files, Opuza will present you a invoice to be paid with Monero
that you must pay before downloading the file.
These invoices can be paid with a Monero wallet.
Popular wallets include:

- [Monero GUI Wallet](https://www.getmonero.org/downloads/), An open-source graphical user interface (GUI) wallet developed by the Monero community
- [Feather](https://featherwallet.org/), Feather is a free, open-source Monero wallet for Linux, Tails, Windows and macOS.
- [Monerujo](https://monerujo.io/), a self-custodial wallet for Android.
- [Cake Wallet](https://cakewallet.com/), a self-custodial wallet for iOS and Android.

## Selling Files with Opuza

Opuza is not a hosted platform.
If you want to sell files through it, you'll have to host your own Opuza instance.
Opuza instances require access to an [monero-wallet-rpc](https://www.getmonero.org/resources/developer-guides/wallet-rpc.html) instance
to create invoices and query their payment status.
monero-wallet-rpc in turn needs access to a [ `Monero node`](https://web.getmonero.org/downloads/) --
to query the state of the Monero blockchain.

### Setting up `monerod` and `monero-wallet-rpc`

A guide to setup a node can be found [here](https://sethforprivacy.com/guides/run-a-monero-node/)

### Processing Payments with Opuza

In order to process payments, Opuza needs to be connected to an monero-wallet-rpc instance.
See the `--monero-*` flags in `opuza --help`.

## Development

You can run the tests locally with `cargo test`.
Pull requests are tested on github actions, with the workflow defined in `.github/workflows/build.yaml`.
You can run approximately the same tests locally with `just all`.
(See [just](https://github.com/casey/just).)

## License

Opuza is licensed under [the CC0](https://choosealicense.com/licenses/cc0-1.0) with the exception of third-party components listed in [`ATTRIBUTION.md`](ATTRIBUTION.md).
