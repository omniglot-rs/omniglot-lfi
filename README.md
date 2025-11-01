# Work-in-progress LFI Runtime for Omniglot

This is a work-in-progress Omniglot runtime using [Lightweight Fault
Isolation](https://github.com/lfi-project). It can isolate untrusted libraries
and interact with them from safe Rust, maintaining both memory- and type-safety.

## Development Setup

The easiest way to get setup and install all dependencies is using Nix. This
project contains a `flake.nix` file which can be used to build the project,
ensure that it complies with various formatting and linter checks, and enter
into an interactive development shell.

First, be sure to have a multi-user Nix installed. You can do so with the
following command, run as a regular non-root user (it will prompt you whether it
is allowed to use sudo). Follow the prompts to install it, and then re-enter or
re-source your shell and ensure that you have the `nix` binary available in your
path:

```
$ sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon
```

Nix flakes aren't yet stabilized, so you need to add an option to your
`nix.conf`. You can do so using this command:

```
$ echo "experimental-features = nix-command flakes" | sudo tee -a /etc/nix/nix.conf
```

Finally, you can test whether everything works by running the following command
in this repository:

```
$ nix run '.#omniglot-lfi-example-add'
```

This will take a few minutes to fetch all dependencies and build
Omniglot. Eventually, you should see the following output:

```
$ nix run '.#omniglot-lfi-example-add'
add(1, 2) = 3
```

With Nix installed and ready, you can use the following hany commands:

```
$ nix develop      # Enter into a development shell, with all dependencies available
$ nix fmt          # Format source files in tree (using rustfmt, nixfmt, etc.)
$ nix flake check  # Run basic formatting checks and linters
$ nix flake show   # Show targets available in this Nix flake
```
