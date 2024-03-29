# homesync

**Caution! This was an experimental project**

This project was an exploration in the Rust language, which I do not plan on
exploring any further. I do want to rewrite this tool properly (in C) at a
later date, but I have no definitive timeline on when I'll start.

## Introduction

Homesync provides a way of automatically syncing files across your desktop to a
<code>git</code> repository. By registering files with homesync, upstream and
downstream syncing can be done as requested, without any manual need to twiddle
files and copy them into a local git repository. Separately, a daemon service
exists that watches for changes as they happen.

## Installation

Currently `nix` is the only distribution channel available. You can run directly
from the shell like so:

```bash
$ nix shell github:jrpotter/homesync/main
```

Likewise, you can install the binary using [home-manager](https://github.com/nix-community/home-manager):

```nix
{
  inputs = {
    homesync.url = "github:jrpotter/homesync/main";
  };

  # ...

  configuration = { ... }: {
    home.packages = [
      homesync.defaultPackage.${system}
    ];
  };
}
```

Lastly, you can always just build from source:

```bash
$ git clone https://github.com/jrpotter/homesync.git
$ cd homesync
$ cargo build --release
```

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
unmanaged:
  - LICENSE
  - README.md
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
can run either:

```bash
$ homesync apply <PACKAGE>
$ homesync apply --all
```

The latter command overwrites all configuration files in favor of those found in
the local repository.

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

To get started, we recommend reviewing the documentation:

```bash
cargo doc --open
```

Install git hooks as follows:

```bash
git config --local core.hooksPath .githooks/
```
