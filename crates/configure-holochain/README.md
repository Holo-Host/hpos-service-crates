# hpos-configure-holochain

## Ownership Info

Codeowner: @zo-el
Consulted: @peeech
Informed: None

Installs apps in holochain and downloads UI in the `UI_STORE_FOLDER` directory listed in YAML configuration file. Also basic holochain clean-up is performed (see below).

Optionally if environmental variable `HOST_PUBKEY_PATH` is set the holoport's host public key created during first run will be saved in a file at given path and retrieved during subsequent runs.

## Usage

```
$ hpos-configure-holochain --help
USAGE:
    hpos-configure-holochain [OPTIONS] <happ-list-path> --ui-store-folder <ui-store-folder>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --admin-port <admin-port>              Holochain conductor port [env: ADMIN_PORT=]  [default: 4444]
        --happ-port <happ-port>                hApp listening port [env: HAPP_PORT=]  [default: 42233]
        --ui-store-folder <ui-store-folder>    Path to the folder where hApp UIs will be extracted [env:
                                               UI_STORE_FOLDER=]

ARGS:
    <happ-list-path>    Path to a YAML file containing the list of hApps to install

```

where file at `happ-list-path` is of a format:

```yaml
core_happs: [Happ]
self_hosted_happs: [Happ]
```

where `Happ` is

```yaml
app_id: string
version: string
dna_url: string (optional)
ui_url: string (optional)
```

Example YAML:

```yaml
---
core_happs:
  - app_id: hha
    version: 1
    dna_url: https://s3.eu-central-1.wasabisys.com/elemetal-chat-tests/hha.dna.gz
self_hosted_happs:
  - app_id: elemental-chat
    version: 1
    dna_url: https://github.com/holochain/elemental-chat/releases/download/v0.0.1-alpha3/elemental-chat.dna.gz
    ui_url: https://github.com/holochain/elemental-chat-ui/releases/download/v0.0.1-alpha7/elemental-chat.zip
```

## Basic clean-up

At the runtime script deactivates all the apps in holochain that **DO NOT** meet the criteria:

`app_id` contains string `:hCAk` OR `app_id` is listed in YAML configuration file

With such a condition the only apps that remain active are self-hosted and core happs installed from [HPOS configuration](https://github.com/Holo-Host/holo-nixpkgs/blob/develop/profiles/logical/hpos/default.nix#L203) and hosted happs installed by [envoy](https://github.com/Holo-Host/holo-envoy).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
