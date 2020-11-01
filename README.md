# self-hosted-happs
Install self-hosted hApps from a YAML file into holochain

## Usage

```
self-hosted-happs config.yaml
```

where `config.yaml` is of a format `[{app_id, ui_url, dna_url}]`:

```yaml
---
- app_id: elemental-chat
  ui_url: https://s3.eu-central-1.wasabisys.com/elemetal-chat-tests/elemental-chat.zip
  dna_url: https://s3.eu-central-1.wasabisys.com/elemetal-chat-tests/elemental-chat.dna.gz
```

All options can be set as a CLI flag or an environment variable.
See `self-hosted-happs --help` for information on what options are available.
