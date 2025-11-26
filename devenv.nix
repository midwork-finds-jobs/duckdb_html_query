{ pkgs, lib, config, inputs, ... }:

{
  languages.opentofu.enable = true;

  languages.rust = {
    enable = true;
    channel = "stable";
    targets = [ "aarch64-unknown-linux-gnu" "x86_64-unknown-linux-gnu" ];
  };

  packages = with pkgs; [
    cargo-lambda
  ];

  pre-commit.hooks = {
    rustfmt = {
      enable = true;
      entry = "cargo fmt --all --check";
    };

    clippy = {
      enable = true;
      entry = "cargo clippy --all-targets --all-features -- -D warnings";
    };

    cargo-test = {
      enable = true;
      entry = "cargo test --all-features";
    };
  };
}
