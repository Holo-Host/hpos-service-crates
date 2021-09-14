echo 'cleaning up tmp from any previous run'
rm -rf test/tmp
mkdir test/tmp
mkdir test/tmp/databases
mkdir test/tmp/keystore
mkdir test/tmp/uis
echo 'starting holochain'
holochain -c test/holochain-config.yaml &
echo 'running configure-holochain'
RUST_LOG=debug cargo run --bin configure-holochain -- test/config.yaml test/mem-proof --ui-store-folder test/tmp/uis
killall holochain
