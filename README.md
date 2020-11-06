# hpos-configure-holochain
Configure holochain for HPOS and install self-hosted hApps from a YAML file

## Usage

```
hpos-configure-holochain config.yaml
```

where `config.yaml` is of a format `[{app_id, ui_url, dna_url}]`:

```yaml
---
- app_id: elemental-chat
  ui_url: https://s3.eu-central-1.wasabisys.com/elemetal-chat-tests/elemental-chat.zip
  dna_url: https://s3.eu-central-1.wasabisys.com/elemetal-chat-tests/elemental-chat.dna.gz
```

All options can be set as a CLI flag or an environment variable.
See `hpos-configure-holochain --help` for information on what options are available.
