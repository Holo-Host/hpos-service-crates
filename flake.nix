{
  description = "Template for Holochain app development";

    # this can now be udpated directly, e.g.:
    # nix flake lock --override-input holochain github:holochain/holochain/holochain-0.1.3

  inputs = {
    versions.url = "github:holochain/holochain?dir=versions/0_2";
    versions.inputs.holochain.url = "github:holochain/holochain/holochain-0.2.2-beta-rc.1";
    versions.inputs.lair.url = "github:holochain/lair/lair_keystore-v0.3.0";
 
    holochain-flake.url = "github:holochain/holochain";
    holochain-flake.inputs.versions.follows = "versions";

    nixpkgs.follows = "holochain-flake/nixpkgs";
    flake-parts.follows = "holochain-flake/flake-parts";
     # inputs.holochain.follows = "holochain";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; }
      {
        systems = builtins.attrNames inputs.holochain-flake.devShells;

        perSystem =
          { inputs'
          , config
          , pkgs
          , system
          , ...
          }: {

            devShells.default = pkgs.mkShell {
              inputsFrom = [ inputs'.holochain-flake.devShells.holonix ];
              packages = [
                pkgs.nodejs-18_x
                pkgs.binaryen
                # more packages go here
              ];
            };
          };
      };
}