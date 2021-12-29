# homesync

**Caution! This is unstable code!**

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
Locations are searched in the following order:

- `$XDG_CONFIG_HOME/homesync/homesync.yml`
- `$XDG_CONFIG_HOME/homesync.yml`
- `$HOME/.config/homesync/homesync.yml`
- `$HOME/.homesync.yml`

That said, it is recommended to modify this config solely from the exposed
homesync CLI. Homesync will take responsibility ensuring how the config is
modified based on your package manager, platform, etc.

## Usage

TODO

## Contribution

Install git hooks as follows:

```bash
git config --local core.hooksPath .githooks/
```
