{ pkgs, inputs, ... }:

{
  languages.rust = {
    enable = true;
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
    toolchain = {
      rustfmt = inputs.fenix.packages.${pkgs.stdenv.hostPlatform.system}.latest.rustfmt;
    };
  };

  git-hooks.hooks = {
    clippy.enable = true;
  };

  cachix.enable = false;
  dotenv.disableHint = true;
}
