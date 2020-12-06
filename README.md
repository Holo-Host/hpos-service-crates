# hpos-configure-holochain
Install self-hosted hApps from a YAML file into holochain

## Usage

```
$ hpos-configure-holochain --help
USAGE:
    self-hosted-happs [OPTIONS] <happ-list-path> --ui-store-folder <ui-store-folder>

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

where `happ-list-path` is of a format:

```yaml
---
- installed_app_id: elemental-chat
  version: 1
  ui_url: https://github.com/holochain/elemental-chat-ui/releases/download/v0.0.1-alpha7/elemental-chat.zip
  dna_url: https://github.com/holochain/elemental-chat/releases/download/v0.0.1-alpha3/elemental-chat.dna.gz
```
