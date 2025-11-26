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
}
