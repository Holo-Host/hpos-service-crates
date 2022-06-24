# This file was generated with the following command:
# update-holochain-versions --git-src=revision:holochain-0.0.145 --lair-version-req=~0.1 --output-file=holochain_version.nix
# For usage instructions please visit https://github.com/holochain/holochain-nixpkgs/#readme

{
    url = "https://github.com/holochain/holochain";
    rev = "holochain-0.0.145";
    sha256 = "sha256-NyhR+Sa7B/pi7Wou9A0EoRndoChTJudjhmaBm5zN86I=";
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

    rustVersion = "1.61.0";

    lair = {
        url = "https://github.com/holochain/lair";
        rev = "lair_keystore_api-v0.1.3";
        sha256 = "sha256-1amhBe34dEOlTATryHdKaz/NMUk2Mnn79VrahvO4OnY=";

        binsFilter = [
            "lair-keystore"
        ];

        rustVersion = "1.61.0";

        cargoLock = {
            outputHashes = {
            };
        };
    };
}
