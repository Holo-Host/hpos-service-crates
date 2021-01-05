# hpos-configure-holochain
Installs apps in holochain and downloads UI in the `UI_STORE_FOLDER` directory from a YAML configuration file.

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
