# rs4acap

_A collection of language agnostic tools that facilitate development for the AXIS Camera Application Platform (ACAP)._

## Table of Contents

- [Preview](#preview)
  - [Porcelain programs](#porcelain-programs)
    - [cli4a](#cli4a)
  - [Plumbing programs](#plumbing-programs)
    - [device-finder](#device-finder)
    - [device-inventory](#device-inventory)
    - [device-manager](#device-manager)
    - [firmware-inventory](#firmware-inventory)
- [Installation](#installation)
- [Testing](#testing)
- [Related projects](#related-projects)

## Preview

The tools in this project are split into two categories:

- **Porcelain programs** combine one or more plumbing programs to make common workflows ergonomic.
  Because integrations change and break, porcelain is inherently more fragile than the plumbing it sits on.
  To prevent combinatorial growth of the test matrix, only a subset of the plumbing functionality is exposed.
- **Plumbing programs** each target a single integration.
  These are designed to be composable, robust, and to afford the user more flexibility than porcelain programs.

Note that some programs don't yet fit this model.
The biggest deviation is in `device-inventory`, which integrates with mDNS and the VLT in addition to its own database.
The status of `firmware-inventory` is also a bit murky as it integrates with an online service and its own database.

### Porcelain programs

#### `cli4a`

```console
$ cli4a help
Usage: cli4a <COMMAND>

Commands:
  upgrade      Upgrade the device to a firmware version matching a semver requirement
  completions  Generate shell completions
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### Plumbing programs

#### `device-finder`

```console
$ device-finder help
Usage: device-finder [COMMAND]

Commands:
  discover-devices  Discover devices on the local network
  completions       Print a completion file for the given shell
  help              Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

#### `device-inventory`

```console
$ device-inventory help
Usage: device-inventory [OPTIONS] <COMMAND>

Commands:
  login        Login to a pool of shared devices
  add          Add a device
  deactivate   Deactivate any active device
  sync         Sync devices from the VLT to the local inventory
  for-each     Run a command with environment variables set for each device
  list         List available devices
  activate     Activate an existing device
  remove       Remove a device
  dump         Print the device-inventory database to stdout
  load         Load the device-inventory database from stdin
  completions  Print a completion file for the given shell
  help         Print this message or the help of the given subcommand(s)

Options:
      --inventory <INVENTORY>  Location of the application data [env: DEVICE_INVENTORY_LOCATION=]
      --offline                [env: DEVICE_INVENTORY_OFFLINE=]
  -h, --help                   Print help
```

#### `device-manager`

```console
$ device-manager help
Usage: device-manager <COMMAND>

Commands:
  restore      Restore the device to a clean state (factory default)
  init         Initialize a device in setup mode
  reinit       Restore and initialize the device to a known, useful state
  upgrade      Upgrade the device firmware
  completions  Generate shell completions
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

#### `firmware-inventory`

```console
$ firmware-inventory help
Usage: firmware-inventory [OPTIONS] <COMMAND>

Commands:
  login        Login to access firmware downloads
  update       Update the local firmware index from the upstream catalog
  list         List indexed firmware versions, showing which are cached locally
  get          Get firmware for each product, matching a version requirement or a track
  completions  Print a completion file for the given shell
  help         Print this message or the help of the given subcommand(s)

Options:
      --inventory <INVENTORY>  Location of the application data [env: FIRMWARE_INVENTORY_LOCATION=]
      --offline                [env: FIRMWARE_INVENTORY_OFFLINE=]
  -h, --help                   Print help
```

## Installation

The tools in this project can be installed using Cargo:

```shell
cargo install --locked --git https://github.com/apljungquist/rs4a.git cli4a
cargo install --locked --git https://github.com/apljungquist/rs4a.git device-finder
cargo install --locked --git https://github.com/apljungquist/rs4a.git device-inventory
cargo install --locked --git https://github.com/apljungquist/rs4a.git rs4a-device-manager
cargo install --locked --git https://github.com/apljungquist/rs4a.git rs4a-firmware-inventory
```

Prebuilt binaries are also published on the [Releases page](https://github.com/apljungquist/rs4a/releases) as they become available.
On macOS, binaries downloaded this way are unsigned, so Gatekeeper will refuse to run them until you clear the quarantine flag:

```shell
xattr -d com.apple.quarantine /path/to/binary
```

If you want to install them another way, open an issue and I may be able to help.

## Testing

These tools are tested by my own use, primarily on macOS, occasionally on Linux, and never on Windows.
Feedback is welcome in the GitHub discussions, or wherever else you can find me.

## Related projects

- [acap-rs](https://github.com/AxisCommunications/acap-rs) - though focused on facilitating developing ACAP apps in Rust, this project does provide a few language agnostic tools too.
- [awesome-acap](https://github.com/apljungquist/awesome-acap) - a list of free resources related to ACAP development.
