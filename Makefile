# Easily update the crates to the lastest versions of the Holochain ecosystem
# Step 1:
# Update the ./version-manager.json file with the latest versions of the crates that you intend to update
# Step 2:
# Update the flake to the version of the Holochain that you want to update to
# Step 3:
# Run `nix flake update` to update the flake
# Step 4:
# Run `make update` to update the crates to the latest versions
# Step 5:
# Run `nix develop` to update the nix shell with the latest versions of the crates
# Step 6:
# Run `cargo build` to build the project with the latest versions of the crates
update:
	rm -rf Cargo.lock
	echo '⚙️  Updating crate...'
	cargo upgrade \
		-p holochain_types@$(shell jq .holochain_types ./version-manager.json) \
		-p holofuel_types@$(shell jq .holofuel_types ./version-manager.json) \
		-p holochain_keystore@$(shell jq .holochain_keystore ./version-manager.json) \
		-p lair_keystore_api@$(shell jq .lair_keystore_api ./version-manager.json) \
		-p holochain_conductor_api@$(shell jq .holochain_conductor_api ./version-manager.json) \
		-p holochain_websocket@$(shell jq .holochain_websocket ./version-manager.json) \
		-p mr_bundle@$(shell jq .mr_bundle ./version-manager.json)  --pinned