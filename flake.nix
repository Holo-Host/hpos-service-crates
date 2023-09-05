{
  inputs = {
    nixpkgs.follows = "holochain/nixpkgs";
    holochain = {
      url = "github:holochain/holochain";
      inputs.versions.url = "github:holochain/holochain?dir=versions/0_2";
      inputs.holochain.url = "github:holochain/holochain/holochain-0.2.2-beta-rc.1";
    };
  };

  outputs = inputs @ { ... }:
    inputs.holochain.inputs.flake-parts.lib.mkFlake { inherit inputs; }
      {
        systems = builtins.attrNames inputs.holochain.devShells;
        perSystem =
          { config
          , pkgs
          , system
          , ...
          }: {
            devShells.default = pkgs.mkShell {
              inputsFrom = [ inputs.holochain.devShells.${system}.holonix ];
              packages = [ pkgs.nodejs-18_x pkgs.binaryen  ];
            };
          };
      };
}