# homesync

**Caution! This is a work in progress and far from complete!**

## Introduction

Homesync provides a way of automatically syncing config files across various
applications you may use. It works by establishing a file watcher on all the
configs specified in the primary `homesync` config. As files are changed, they
are copied to a local git repository to eventually be pushed by the user.
Likewise, at any point, the user can sync against the remote repository,
overwriting local configurations for one or more packages.

## Installation

TODO

## Configuration

Homesync uses a YAML file, to be found in anyone of the following locations.
Locations are searched in the following priority:

- `$HOME/.homesync.yml`
- `$HOME/.config/homesync/homesync.yml`
- `$XDG_CONFIG_HOME/homesync.yml`
- `$XDG_CONFIG_HOME/homesync/homesync.yml`

The config file should look like the following:

```yaml
---
user:
  name: name
  email: email@email.com
ssh:
  public: $HOME/.ssh/id_ed25519.pub
  private: $HOME/.ssh/id_ed25519
repos:
  local: $HOME/.homesync
  remote:
    name: origin
    branch: master
    url: "https://github.com/owner/repo.git"
packages:
  homesync:
    - $HOME/.homesync.yml
    - $HOME/.config/homesync/homesync.yml
    - $XDG_CONFIG_HOME/homesync.yml
    - $XDG_CONFIG_HOME/homesync/homesync.yml
```

Copy over [examples/template.yaml](https://github.com/jrpotter/homesync/blob/main/examples/template.yaml)
to where you'd like as a starting point.

## Usage

Verify your installation by running `homesync` from the command line. To have
your local repository match the remote, run

```bash
$ homesync pull
```

If you make a change to a configuration tracked by homesync, you can tell
homesync to prep pushing those changes via the `stage` subcommand or rely on
the daemon service to do it for you:

```bash
$ homesync stage
$ homesync daemon &
```

Homesync will find all tracked files that have changed and stage them in the
local repository. You can then push those changes using

```bash
$ homesync push
```

If looking to copy a configuration tracked by homesync to your desktop, you
can run

```bash
$ homesync apply <filename>
```

To copy all configurations (and optionally overwrite files that already exist),
you can run

```bash
$ homesync apply --all [--overwrite]
```

## Known Issues

If using (neo)vim, the daemon watcher will stop watching a given configuration
file after editing. Refer to [this issue](https://github.com/notify-rs/notify/issues/247)
for more details. As a workaround, you can set the following in your `init.vim`
file:

```vimscript
backupcopy=yes
```

Refer to `:h backupcopy` for details on how this works.

## Contribution

Install git hooks as follows:

```bash
git config --local core.hooksPath .githooks/
```
