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

### Using a local, custom build of `lfi-runtime`

For development it can be useful for build or tests against a custom build of
`lfi-runtime`, and not the version built using included Nix derivation. For this
purpose, the Nix flake features a special `external-lfi-runtime` development
shell.

First, build your local checkout of the `lfi-runtime` repository and then
"install" it into a local directory, as follows:
```
~/dev $ git clone https://github.com/lfi-project/lfi-runtime
~/dev $ cd lfi-runtime/

# If on NixOS, make build dependencies available in a Nix shell:
~/dev/lfi-runtime $ nix-shell -p meson ninja pkg-config

~/dev/lfi-runtime $ mkdir -p install
~/dev/lfi-runtime $ meson setup --prefix=$(readlink -f ./install) build
~/dev/lfi-runtime $ cd build/
~/dev/lfi-runtime/build $ ninja
~/dev/lfi-runtime/build $ ninja install
```

Now, run the following command in the `omniglot-lfi` repository to enter into a
development shell that uses this built LFI runtime:
```
~/dev/omniglot-lfi $ LFI_RUNTIME_INSTALL_PREFIX=$(readlink -f ../lfi-runtime/install/) \
    nix develop '.#external-lfi-runtime'
```

Within this shell, you can build the Omniglot LFI runtime and run examples and tests as usual:
```
~/dev/omniglot-lfi $ cargo test
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/omniglot_lfi_tests-537ec127019f5fc2)
running 11 tests
test add::test_add5 ... ok
test add::test_add3 ... ok
test add::test_add4 ... ok
[...]
```
