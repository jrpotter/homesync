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

That said, it is recommended to modify this config solely from the exposed
homesync CLI. Homesync will take responsibility ensuring the generated
configuration is according to package manager, platform, etc.

## Usage

Verify your installation by running `homesync` from the command line. If
installed, you will likely want to initialize a new config instance. Do so by
typing:

```bash
$ homesync init
```

You can then walk through what github repository you want to sync your various
files with. You can have homesync automatically monitor all configuration files
and post updates on changes by running

```bash
$ homesync daemon
```

As changes are made to your `homesync` config or any configuration files
referred to within the `homesync` config, the daemon service will sync the
changes to the local git repository. To push these changes upward, run

```bash
$ homesync push --all
```

which will expose a git interface for you to complete the push. Lastly, to sync
the remote configurations to your local files, run

```bash
$ homesync pull --all
```

This will load up a diff wrapper for you to ensure you make the changes you'd
like.

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
