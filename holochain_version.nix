# This file was generated with the following command:
# update-holochain-versions --git-src=revision:holochain-0.1.0-beta-rc.2 --output-file=holochain_version.nix
# For usage instructions please visit https://github.com/holochain/holochain-nixpkgs/#readme

{
    url = "https://github.com/holochain/holochain";
    rev = "holochain-0.1.0-beta-rc.2";
    sha256 = "sha256-GbjO3YANNDnWR2g2LBkSAgzwfc6Lrji5T/8xg+lTTCw=";
    cargoLock = {
        outputHashes = {
        };
    };

    binsFilter = [
        "holochain"
        "hc"
        "kitsune-p2p-proxy"
        "kitsune-p2p-tx2-proxy"
    ];


    lair = {
        url = "https://github.com/holochain/lair";
        rev = "lair_keystore_api-v0.2.3";
        sha256 = "sha256-cqOr7iWzsNeomYQiiFggzG5Dr4X0ysnTkjtA8iwDLAQ=";

        binsFilter = [
            "lair-keystore"
        ];


        cargoLock = {
            outputHashes = {
            };
        };
    };
}
